use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use claviron::http::boxed_body;
use claviron::http::ext::ConnCtx;
use claviron::http::session::{ConnectionScope, SessionOwner};
use claviron::runtime::registry::{self, ProviderOrder};
use claviron::runtime::{AppState, Provider, RunContext, Runnable};
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder;
use tokio::net::{TcpListener, UnixListener};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::application::{self, Application};

pub(crate) struct SocketIngress {
    tcp: Option<SocketAddr>,
    uds: Option<PathBuf>,
}
impl SocketIngress {
    pub fn new(
        tcp: Option<SocketAddr>,
        uds: Option<PathBuf>,
    ) -> Self {
        Self { tcp, uds }
    }
}
impl Provider<AppState> for SocketIngress {
    fn name(&self) -> &'static str {
        "LumenizeSocketIngress"
    }

    fn order(&self) -> ProviderOrder {
        ProviderOrder::new().after::<Application>()
    }

    fn as_runnable(self: Arc<Self>) -> Option<Arc<dyn Runnable<AppState>>> {
        Some(self)
    }
}

// Never unlink a pre-existing socket: it may belong to a live process of another name.
struct SocketFile {
    path: PathBuf,
    device: u64,
    inode: u64,
}
impl SocketFile {
    fn owned(path: PathBuf) -> std::io::Result<Self> {
        use std::os::unix::fs::MetadataExt;
        let meta = std::fs::symlink_metadata(&path)?;
        Ok(Self { path, device: meta.dev(), inode: meta.ino() })
    }
}
impl Drop for SocketFile {
    fn drop(&mut self) {
        use std::os::unix::fs::MetadataExt;
        if let Ok(meta) = std::fs::symlink_metadata(&self.path)
            && meta.dev() == self.device
            && meta.ino() == self.inode
        {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}
#[async_trait]
impl Runnable<AppState> for SocketIngress {
    async fn run(
        self: Arc<Self>,
        state: AppState,
        context: RunContext,
    ) -> registry::Result<()> {
        let app = application::resolve(&state)?;
        let tcp = match self.tcp {
            Some(addr) => Some(TcpListener::bind(addr).await.map_err(failure)?),
            None => None,
        };
        let uds = match &self.uds {
            Some(path) => Some(UnixListener::bind(path).map_err(failure)?),
            None => None,
        };
        let _socket_file = self
            .uds
            .clone()
            .map(SocketFile::owned)
            .transpose()
            .map_err(failure)?;
        if let Some(listener) = &tcp {
            tracing::info!(address = %listener.local_addr().map_err(failure)?, "Lumenize TCP ready");
        }
        if let Some(path) = &self.uds {
            tracing::info!(path = %path.display(), "Lumenize Unix socket ready");
        }
        let mut connections = JoinSet::new();
        let sessions = SessionOwner::new("lumenize".parse().expect("valid listener identity"));
        // Admission never waits here; the unbounded gate carries shutdown and
        // keeps a connection's lease alive after a WebSocket hand-off.
        let gate = claviron::runtime::Gate::new(None, std::time::Duration::ZERO);
        let stop = CancellationToken::new();
        let _cancel_on_drop = stop.clone().drop_guard();
        let mut error = None;
        loop {
            tokio::select! {
                biased;
                () = context.cancelled() => break,
                accepted = async { tcp.as_ref().unwrap().accept().await }, if tcp.is_some() => {
                    match accepted {
                        Ok((stream, peer)) => {
                            let local = match stream.local_addr() { Ok(local) => local, Err(e) => { error = Some(e); break; } };
                            let scope = sessions.bind(gate.try_enter().map_err(failure)?);
                            connections.spawn(serve(stream, Some((peer, local)), app.clone(), stop.clone(), Some(scope)));
                        },
                        Err(e) => { error = Some(e); break; }
                    }
                },
                accepted = async { uds.as_ref().unwrap().accept().await }, if uds.is_some() => {
                    match accepted {
                        Ok((stream, _)) => {
                            let scope = sessions.bind(gate.try_enter().map_err(failure)?);
                            connections.spawn(serve(stream, None, app.clone(), stop.clone(), Some(scope)));
                        },
                        Err(e) => { error = Some(e); break; }
                    }
                },
                Some(done) = connections.join_next(), if !connections.is_empty() => { report(done); }
            }
        }
        drop(tcp);
        drop(uds);
        let sessions = sessions.close();
        gate.graceful_shutdown(None);
        stop.cancel();
        // Idle keep-alive connections receive graceful_shutdown too; no invented timer.
        while let Some(done) = connections.join_next().await {
            report(done);
        }
        sessions.join().await;
        if let Some(error) = error {
            return Err(failure(error));
        }
        Ok(())
    }
}
fn report(done: Result<std::io::Result<()>, tokio::task::JoinError>) {
    if !matches!(done, Ok(Ok(()))) {
        tracing::debug!(?done, "Lumenize connection ended");
    }
}
async fn serve<T>(
    stream: T,
    peers: Option<(SocketAddr, SocketAddr)>,
    app: Arc<Application>,
    stop: CancellationToken,
    scope: Option<ConnectionScope>,
) -> std::io::Result<()>
where
    T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let session_spawner = scope.as_ref().map(ConnectionScope::sessions);
    let service = service_fn(move |request: http::Request<Incoming>| {
        let app = app.clone();
        let session_spawner = session_spawner.clone();
        async move {
            let mut request = request.map(boxed_body);
            request.extensions_mut().insert(http::uri::Scheme::HTTP);
            if let Some(sessions) = session_spawner {
                request.extensions_mut().insert(sessions);
            }
            {
                // Unix sockets have no IP peer; unspecified addresses carry
                // that absence without trusting forwarded client headers.
                let unspecified = SocketAddr::from(([0, 0, 0, 0], 0));
                let (remote, local) = peers.unwrap_or((unspecified, unspecified));
                request.extensions_mut().insert(Arc::new(ConnCtx {
                    listener: "lumenize".parse().expect("valid listener identity"),
                    proto: None,
                    remote,
                    local_ip: local.ip(),
                    local_port: local.port(),
                    real_ip: None,
                    country_code: None,
                    tls: None,
                }));
            }
            Ok::<_, Infallible>(app.handle(request).await)
        }
    });
    let builder = Builder::new(TokioExecutor::new());
    let connection = builder.serve_connection_with_upgrades(TokioIo::new(stream), service);
    tokio::pin!(connection);
    tokio::select! {
        result = &mut connection => result.map_err(|e| std::io::Error::other(e.to_string())),
        () = stop.cancelled() => {
            connection.as_mut().graceful_shutdown();
            connection.await.map_err(|e| std::io::Error::other(e.to_string()))
        }
    }
}
fn failure(e: impl std::fmt::Display) -> registry::Error {
    registry::Error::msg(e.to_string())
}

#[cfg(test)]
#[path = "../tests/socket.rs"]
mod tests;
