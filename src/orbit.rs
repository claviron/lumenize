use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use claviron::http::ext::{ClientAuthStatus, ConnCtx, TlsInfo};
use claviron::http::{RouteBody, boxed_body, empty_body};
use claviron::orbit::transport::{
    Exchange, ExchangeReply, TRANSPORT_BUFFER_BYTES, TransportBodies, TransportSegment,
    parking_supported,
};
use claviron::runtime::registry::{self, ProviderOrder};
use claviron::runtime::{AppState, FleetIncarnation, Provider, RunContext, Runnable};
use http::{Request, StatusCode, Uri};
use http_body_util::{BodyExt, StreamBody};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tokio_stream::StreamExt;

use crate::application::{self, Application};

pub(crate) struct OrbitIngress {
    name: String,
    max_in_flight: usize,
}
impl OrbitIngress {
    pub fn new(
        name: String,
        max_in_flight: usize,
    ) -> Self {
        Self { name, max_in_flight }
    }
}
impl Provider<AppState> for OrbitIngress {
    fn name(&self) -> &'static str {
        "LumenizeOrbitIngress"
    }

    fn order(&self) -> ProviderOrder {
        ProviderOrder::new().after::<Application>()
    }

    fn as_runnable(self: Arc<Self>) -> Option<Arc<dyn Runnable<AppState>>> {
        Some(self)
    }
}
struct Lane {
    segment: Arc<TransportSegment>,
    index: usize,
    stop: Arc<AtomicBool>,
}
impl Drop for Lane {
    fn drop(&mut self) {
        let _ = self.segment.inbox().release_lane(self.index);
        self.stop.store(true, Ordering::Release);
        let _ = self.segment.inbox().nudge(self.index);
    }
}
#[async_trait]
impl Runnable<AppState> for OrbitIngress {
    async fn run(
        self: Arc<Self>,
        state: AppState,
        context: RunContext,
    ) -> registry::Result<()> {
        if !parking_supported() {
            return Err(failure(
                "Orbit requires a native shared-memory wait primitive",
            ));
        }
        let app = application::resolve(&state)?;
        let incarnation = state
            .registry_ref()
            .resolve::<FleetIncarnation>()
            .map_or(FleetIncarnation::FIRST, |v| *v)
            .raw();
        let segment = Arc::new(TransportSegment::for_application(&self.name).map_err(failure)?);
        let index = segment
            .inbox()
            .join(&self.name, "php", incarnation)
            .map_err(failure)?;
        let stop = Arc::new(AtomicBool::new(false));
        let lane = Lane { segment: segment.clone(), index, stop: stop.clone() };
        let bodies = Arc::new(
            TransportBodies::for_application(&self.name, index, incarnation).map_err(failure)?,
        );
        // One frame is a handoff, not a second backlog invisible to edge selection.
        let (tx, mut rx) = mpsc::channel(1);
        let reader_segment = segment.clone();
        let reader = std::thread::Builder::new()
            .name(format!("lumenize-orbit-{index}"))
            .spawn(move || {
                let inbox = reader_segment.inbox();
                while !stop.load(Ordering::Acquire) {
                    let (_, expected) = inbox.wait_word(index).map_err(|e| e.to_string())?;
                    let mut frame = Vec::new();
                    while !stop.load(Ordering::Acquire)
                        && inbox.read(index, &mut frame).map_err(|e| e.to_string())?
                    {
                        if tx.blocking_send(std::mem::take(&mut frame)).is_err() {
                            return Ok::<_, String>(());
                        }
                    }
                    if stop.load(Ordering::Acquire) {
                        break;
                    }
                    inbox.park(index, expected).map_err(|e| e.to_string())?;
                }
                Ok(())
            })
            .map_err(failure)?;
        tracing::info!(application = %self.name, lane = index, "Lumenize Orbit ready");
        let mut pending = JoinSet::new();
        loop {
            tokio::select! {
                biased;
                () = context.cancelled() => break,
                frame = rx.recv(), if pending.len() < self.max_in_flight => match frame {
                    Some(frame) => { pending.spawn(serve(frame, app.clone(), segment.clone(), bodies.clone())); },
                    None => break,
                },
                Some(done) = pending.join_next(), if !pending.is_empty() => report(done),
            }
        }
        // End selection and wake the reader. Drain its already accepted handoff
        // before joining; blocking_send must never hold shutdown hostage.
        drop(lane);
        while let Some(frame) = rx.recv().await {
            if pending.len() >= self.max_in_flight
                && let Some(done) = pending.join_next().await
            {
                report(done);
            }
            pending.spawn(serve(frame, app.clone(), segment.clone(), bodies.clone()));
        }
        let reader_result = tokio::task::spawn_blocking(move || reader.join())
            .await
            .map_err(failure)?;
        while let Some(done) = pending.join_next().await {
            report(done);
        }
        reader_result
            .map_err(|_| failure("Orbit reader panicked"))?
            .map_err(failure)
    }
}
fn report(done: Result<crate::Result<()>, tokio::task::JoinError>) {
    match done {
        Ok(Ok(())) => {}
        other => tracing::warn!(?other, "Orbit exchange failed"),
    }
}
async fn serve(
    frame: Vec<u8>,
    app: Arc<Application>,
    segment: Arc<TransportSegment>,
    bodies: Arc<TransportBodies>,
) -> crate::Result<()> {
    let exchange = Exchange::decode(&frame)?;
    let endpoint = exchange
        .body
        .map(|ticket| bodies.accept(ticket))
        .transpose()?;
    if exchange.upgrade {
        return serve_websocket(&exchange, endpoint, &app, &segment).await;
    }
    let (outgoing, body) = match endpoint {
        Some(endpoint) => {
            let (read, write) = endpoint.split();
            let piece = request_piece(&exchange);
            let stream = tokio_util::io::ReaderStream::with_capacity(read, piece)
                .map(|part| part.map(http_body::Frame::data));
            (Some(write), boxed_body(StreamBody::new(stream)))
        }
        None => (None, empty_body()),
    };
    let response = match build_request(&exchange, body) {
        Ok(request) => app.handle(request).await,
        Err(_) => application::status(StatusCode::BAD_REQUEST),
    };
    let (parts, response_body) = response.into_parts();
    let reply = ExchangeReply {
        correlation: exchange.correlation,
        status: parts.status.as_u16(),
        headers: parts
            .headers
            .iter()
            .map(|(k, v)| (k.as_str().as_bytes().to_vec(), v.as_bytes().to_vec()))
            .collect(),
        body: exchange.body,
    };
    let encoded = reply.encode()?;
    let publish = || -> crate::Result<()> {
        segment.inbox().write(exchange.origin as usize, &encoded)?;
        Ok(())
    };
    if let Some(mut outgoing) = outgoing {
        if let Err(error) = send_body(&mut outgoing, response_body, publish).await {
            outgoing.reset();
            return Err(error);
        }
    } else {
        publish()?;
    }
    Ok(())
}

async fn serve_websocket(
    exchange: &Exchange,
    endpoint: Option<claviron::orbitive::stream::Endpoint>,
    app: &Application,
    segment: &TransportSegment,
) -> crate::Result<()> {
    let answer = |status: StatusCode| -> crate::Result<()> {
        let frame = ExchangeReply {
            correlation: exchange.correlation,
            status: status.as_u16(),
            headers: Vec::new(),
            body: exchange.body,
        }
        .encode()?;
        segment.inbox().write(exchange.origin as usize, &frame)?;
        Ok(())
    };
    #[cfg(feature = "broadcast")]
    {
        let Some(mut endpoint) = endpoint else {
            return answer(StatusCode::BAD_REQUEST);
        };
        let request = match build_request(exchange, empty_body()) {
            Ok(request) => request,
            Err(_) => {
                endpoint.shutdown().await?;
                return answer(StatusCode::BAD_REQUEST);
            }
        };
        let session = match app.accept_orbit_websocket(request) {
            Ok(session) => session,
            Err(status) => {
                endpoint.shutdown().await?;
                return answer(status);
            }
        };
        // Retain the admitted Broadcast session and stream inside this ingress's
        // JoinSet. Only then acknowledge; the edge owns the wire handshake.
        answer(StatusCode::SWITCHING_PROTOCOLS)?;
        let _activity = claviron::runtime::activity::ACTIVITY.open_session();
        session
            .run(claviron::broadcast::stream::OrbitMessageStream::new(
                endpoint,
            ))
            .await?;
        Ok(())
    }
    #[cfg(not(feature = "broadcast"))]
    {
        let _ = app;
        if let Some(mut endpoint) = endpoint {
            endpoint.shutdown().await?;
        }
        answer(StatusCode::NOT_FOUND)
    }
}

fn request_piece(exchange: &Exchange) -> usize {
    exchange
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(b"content-length"))
        .and_then(|(_, value)| std::str::from_utf8(value).ok()?.parse::<usize>().ok())
        .map_or(TRANSPORT_BUFFER_BYTES, |length| {
            length.clamp(1, TRANSPORT_BUFFER_BYTES)
        })
}

// One wire protocol: only the publication order differs. A whole response
// within the stream window is ready before the head wakes its reader; a larger
// or unknown-length body must publish the head first to release backpressure.
async fn send_body<W, F>(
    outgoing: &mut W,
    mut body: RouteBody,
    publish: F,
) -> crate::Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
    F: FnOnce() -> crate::Result<()>,
{
    let whole = http_body::Body::size_hint(&body)
        .exact()
        .is_some_and(|size| size <= TRANSPORT_BUFFER_BYTES as u64);
    if whole {
        let bytes = body.collect().await?.to_bytes();
        outgoing.write_all(&bytes).await?;
        outgoing.shutdown().await?;
        publish()?;
    } else {
        publish()?;
        while let Some(frame) = body.frame().await {
            if let Ok(data) = frame?.into_data() {
                outgoing.write_all(&data).await?;
            }
        }
        outgoing.shutdown().await?;
    }
    Ok(())
}

fn build_request(
    exchange: &Exchange,
    body: RouteBody,
) -> crate::Result<Request<RouteBody>> {
    let mut target = exchange.raw_path.clone();
    if !exchange.query.is_empty() {
        target.push(b'?');
        target.extend_from_slice(&exchange.query);
    }
    let uri = Uri::try_from(target.as_slice())?;
    let scheme: http::uri::Scheme = exchange.scheme.parse()?;
    if scheme != http::uri::Scheme::HTTP && scheme != http::uri::Scheme::HTTPS {
        return Err("unsupported HTTP scheme".into());
    }
    let mut request = Request::builder()
        .method(exchange.method.as_bytes())
        .uri(uri)
        .version(exchange.version.into());
    for (key, value) in &exchange.headers {
        request = request.header(key.as_slice(), value.as_slice());
    }
    let mut request = request.body(body)?;
    request.extensions_mut().insert(scheme.clone());
    // These are the edge's original connection facts, carried by the trusted
    // same-uid transport. HTTPS says no more about TLS than the frame contains.
    if let (Some(remote), Some(local)) = (exchange.client, exchange.server) {
        request.extensions_mut().insert(Arc::new(ConnCtx {
            listener: "lumenize-orbit".parse()?,
            proto: None,
            remote,
            local_ip: local.ip(),
            local_port: local.port(),
            real_ip: None,
            country_code: None,
            tls: (scheme == http::uri::Scheme::HTTPS).then_some(TlsInfo {
                protocol_version: None,
                cipher_suite: None,
                client_auth: ClientAuthStatus::NotProvided,
                client_cert: None,
            }),
        }));
    }
    Ok(request)
}
fn failure(error: impl std::fmt::Display) -> registry::Error {
    registry::Error::msg(error.to_string())
}

#[cfg(test)]
#[path = "../tests/orbit.rs"]
mod tests;
