//! Compose Claviron's existing Broadcast endpoints; protocol and native publication
//! remain owned by Claviron. Native listeners and Orbit ingress own their sessions.
use std::collections::HashSet;
use std::sync::Arc;

use claviron::axum::{Router, middleware};
use claviron::broadcast::BroadcastService;
use claviron::http::session::SessionSpawner;
use claviron::runtime::{AppState, registry};
use claviron::upgrade::{H1UpgradeMiddlewareState, H1UpgradeRegistry};

use crate::routing::AppService;

pub(crate) struct Broadcast {
    paths: HashSet<String>,
    service: Arc<BroadcastService>,
}

#[cfg(test)]
#[path = "../tests/broadcast.rs"]
mod tests;

impl Broadcast {
    pub fn wrap(
        state: &AppState,
        fallback: AppService,
    ) -> registry::Result<(Self, AppService)> {
        let service = state
            .registry_ref()
            .resolve::<BroadcastService>()
            .ok_or_else(|| registry::Error::msg("BroadcastService was not registered"))?;
        // This is the application listener's registry identity, not a vhost.
        // The upstream edge owns public host selection.
        let registry = Arc::new(H1UpgradeRegistry::default());
        let upgrade = H1UpgradeMiddlewareState {
            app_state: state.clone(),
            registry: registry.clone(),
            host: "lumenize".parse().expect("valid listener identity"),
        };
        let mut paths = HashSet::new();
        for (name, app) in service.applications().applications() {
            let path = format!("/app/{}", app.key);
            registry.register_handler(
                upgrade.host.clone(),
                path.clone(),
                service.upgrade_handler(name.as_str()),
            )?;
            paths.insert(path);
        }
        let router = Router::new()
            .merge(claviron::broadcast::pusher_routes(service.clone()))
            .fallback_service(fallback)
            .layer(middleware::from_fn_with_state(
                upgrade,
                claviron::upgrade::h1_upgrade_middleware,
            ));
        Ok((Self { paths, service }, AppService::new(router)))
    }

    pub fn accept_orbit(
        &self,
        request: http::Request<claviron::http::RouteBody>,
    ) -> Result<claviron::broadcast::BroadcastSession, http::StatusCode> {
        use http::{StatusCode, header};
        let request = request.map(claviron::axum::body::Body::new);
        if request.method() != http::Method::GET
            || !claviron::upgrade::is_h1_header_upgrade(&request, "websocket")
            || request
                .headers()
                .get(header::SEC_WEBSOCKET_VERSION)
                .and_then(|v| v.to_str().ok())
                != Some("13")
            || request
                .headers()
                .get(header::SEC_WEBSOCKET_KEY)
                .and_then(|v| v.to_str().ok())
                .is_none()
            || claviron::http::forwarding::reject_request_smuggling(request.headers()).is_err()
        {
            return Err(StatusCode::BAD_REQUEST);
        }
        let key = request
            .uri()
            .path()
            .strip_prefix("/app/")
            .ok_or(StatusCode::NOT_FOUND)?;
        let (application, _) = self
            .service
            .applications()
            .application_by_key(key)
            .ok_or(StatusCode::NOT_FOUND)?;
        self.service
            .accept_session(Arc::from(application))
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
    }

    pub fn accepts(
        &self,
        request: &http::Request<claviron::http::RouteBody>,
    ) -> bool {
        request.method() == http::Method::GET
            && request.version() == http::Version::HTTP_11
            && request.extensions().get::<SessionSpawner>().is_some()
            && self.paths.contains(request.uri().path())
    }
}
