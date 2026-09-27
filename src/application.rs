use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use claviron::ephp::{PhpApplicationAdapter, PhpApplicationService};
use claviron::http::admission::{Admission, head_within};
use claviron::http::body::{GuardedBody, declared_length_within, guard_reply};
use claviron::http::policy::HttpPolicy;
use claviron::http::{RouteBody, RouteResponse, empty_body};
use claviron::runtime::registry::{self, ProviderOrder};
use claviron::runtime::{AppState, Provider};
use http::{Request, Response, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

pub(crate) struct Application {
    name: String,
    policy: HttpPolicy,
    router: OnceLock<crate::routing::AppService>,
    admission: Admission,
    #[cfg(feature = "files")]
    files: Option<Arc<crate::Files>>,
    #[cfg(feature = "broadcast")]
    broadcast: OnceLock<crate::broadcast::Broadcast>,
}
impl Application {
    pub fn new(
        name: String,
        policy: HttpPolicy,
        max_concurrent_requests: Option<usize>,
    ) -> Self {
        Self {
            name,
            policy,
            router: OnceLock::new(),
            admission: Admission::limited(max_concurrent_requests),
            #[cfg(feature = "files")]
            files: None,
            #[cfg(feature = "broadcast")]
            broadcast: OnceLock::new(),
        }
    }

    #[cfg(feature = "files")]
    pub(crate) fn with_files(
        mut self,
        files: Option<Arc<crate::Files>>,
    ) -> Self {
        self.files = files;
        self
    }

    fn prepare(
        &self,
        mut request: Request<RouteBody>,
    ) -> Result<Request<RouteBody>, StatusCode> {
        if claviron::http::forwarding::reject_request_smuggling(request.headers()).is_err() {
            return Err(StatusCode::BAD_REQUEST);
        }
        let allowed_upgrade = false;
        #[cfg(feature = "broadcast")]
        let allowed_upgrade =
            allowed_upgrade || self.broadcast.get().is_some_and(|p| p.accepts(&request));
        if (request.headers().contains_key(http::header::UPGRADE) && !allowed_upgrade)
            || request.method() == http::Method::CONNECT
        {
            return Err(StatusCode::NOT_IMPLEMENTED);
        }
        if declared_length_within(request.headers(), self.policy.requests.max_body_size).is_err() {
            return Err(StatusCode::PAYLOAD_TOO_LARGE);
        }
        request.extensions_mut().insert(self.policy);
        let request = request.map(|body| {
            GuardedBody::new(
                body,
                self.policy.requests.max_body_size,
                self.policy.requests.body_read_timeout,
            )
            .boxed_unsync()
        });
        Ok(request)
    }

    pub async fn handle(
        &self,
        request: Request<RouteBody>,
    ) -> RouteResponse {
        let request = match self.prepare(request) {
            Ok(request) => request,
            Err(code) => return status(code),
        };
        let Some(_place) = self
            .admission
            .enter(self.policy.requests.queue_timeout)
            .await
        else {
            return status(StatusCode::SERVICE_UNAVAILABLE);
        };
        let answering = async {
            match self.router.get() {
                Some(router) => match router
                    .clone()
                    .oneshot(request.map(crate::routing::body))
                    .await
                {
                    Ok(reply) => reply.map(crate::routing::route_body),
                    Err(never) => match never {},
                },
                None => status(StatusCode::SERVICE_UNAVAILABLE),
            }
        };
        match head_within(self.policy.requests.head_timeout, answering).await {
            Some(response) => self.finish(response),
            None => status(StatusCode::GATEWAY_TIMEOUT),
        }
    }

    fn finish(
        &self,
        response: RouteResponse,
    ) -> RouteResponse {
        response.map(|body| {
            guard_reply(
                body,
                self.policy.responses.max_body_size,
                self.policy.responses.body_idle_timeout,
            )
        })
    }

    #[cfg(feature = "broadcast")]
    pub(crate) fn accept_orbit_websocket(
        &self,
        request: Request<RouteBody>,
    ) -> Result<claviron::broadcast::BroadcastSession, StatusCode> {
        self.broadcast
            .get()
            .ok_or(StatusCode::SERVICE_UNAVAILABLE)?
            .accept_orbit(request)
    }
}
pub(crate) fn status(code: StatusCode) -> RouteResponse {
    let mut reply = Response::new(empty_body());
    *reply.status_mut() = code;
    reply
}
#[async_trait]
impl Provider<AppState> for Application {
    fn name(&self) -> &'static str {
        "LumenizeApplication"
    }

    fn order(&self) -> ProviderOrder {
        ProviderOrder::new().after::<PhpApplicationService>()
    }

    fn validate(
        &self,
        state: &AppState,
    ) -> registry::Result<()> {
        let runtime = state
            .registry_ref()
            .resolve::<claviron::ephp::PhpEmbedRuntime>()
            .ok_or_else(|| registry::Error::msg("PHP runtime was not registered"))?;
        if !runtime.has_application(&self.name) {
            return Err(registry::Error::msg(format!(
                "PHP application {:?} is not registered",
                self.name
            )));
        }
        Ok(())
    }

    async fn boot(
        &self,
        state: &AppState,
    ) -> registry::Result<()> {
        let service = state
            .registry_ref()
            .resolve::<PhpApplicationService>()
            .ok_or_else(|| registry::Error::msg("PHP application service was not registered"))?;
        let adapter = PhpApplicationAdapter::new(service, self.name.clone());
        let router =
            if let Some(routes) = state.registry_ref().resolve::<crate::routing::RustRoutes>() {
                routes.php.bind(adapter);
                routes.service.clone()
            } else {
                let php = crate::routing::PhpService::new();
                php.bind(adapter);
                crate::routing::AppService::new(php)
            };
        #[cfg(feature = "files")]
        let router = crate::files::wrap(self.files.clone(), router);
        #[cfg(feature = "broadcast")]
        let router = {
            let (broadcast, router) = crate::broadcast::Broadcast::wrap(state, router)?;
            let _ = self.broadcast.set(broadcast);
            router
        };
        let _ = self.router.set(router);
        Ok(())
    }
}
pub(crate) fn resolve(state: &AppState) -> registry::Result<Arc<Application>> {
    state
        .registry_ref()
        .resolve::<Application>()
        .ok_or_else(|| registry::Error::msg("Lumenize application not registered"))
}

#[cfg(test)]
#[path = "../tests/application.rs"]
mod tests;
