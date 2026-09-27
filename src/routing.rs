use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};
use std::task::{Context, Poll};

use claviron::axum::Router;
use claviron::axum::body::Body;
use claviron::axum::handler::Handler;
use claviron::ephp::PhpApplicationAdapter;
use claviron::runtime::{AppState, Provider, registry};
use http::{Request, Response};
use tower::util::BoxCloneSyncService;
use tower::{Service, ServiceExt};

/// Request delivered to a custom service, with a streaming body.
pub type AppRequest = Request<Body>;
/// Response returned by the PHP service.
pub type AppResponse = Response<Body>;
pub(crate) type AppService = BoxCloneSyncService<AppRequest, AppResponse, Infallible>;

pub(crate) struct RustRoutes {
    pub service: AppService,
    pub php: PhpService,
}

/// The named PHP application, available after provider boot. Place it wherever
/// a custom router or Tower service should dispatch to Laravel.
#[derive(Clone)]
pub struct PhpService(Arc<OnceLock<PhpApplicationAdapter>>);

impl PhpService {
    pub(crate) fn new() -> Self {
        Self(Arc::new(OnceLock::new()))
    }

    pub(crate) fn bind(
        &self,
        adapter: PhpApplicationAdapter,
    ) {
        assert!(
            self.0.set(adapter).is_ok(),
            "PHP service is bound once during boot"
        );
    }

    pub async fn handle(
        &self,
        request: AppRequest,
    ) -> AppResponse {
        let adapter = self.0.get().expect("PHP boots before ingress starts");
        let Ok(response) = adapter
            .handle(request.map(claviron::http::boxed_body))
            .await;
        response.map(Body::new)
    }
}

impl Service<AppRequest> for PhpService {
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<AppResponse, Infallible>> + Send>>;
    type Response = AppResponse;

    fn poll_ready(
        &mut self,
        _: &mut Context<'_>,
    ) -> Poll<Result<(), Infallible>> {
        Poll::Ready(Ok(()))
    }

    fn call(
        &mut self,
        request: AppRequest,
    ) -> Self::Future {
        let php = self.clone();
        Box::pin(async move { Ok(php.handle(request).await) })
    }
}

impl Provider<AppState> for RustRoutes {
    fn name(&self) -> &'static str {
        "LumenizeRustRoutes"
    }
}

/// Register the application's Rust routes. Undeclared paths and methods go to
/// PHP; a response produced by a matching Rust handler is returned unchanged.
/// Pass a router without a custom fallback; Lumenize installs the PHP fallback
/// using a handle bound to the shared PHP application service during boot.
/// The builder receives the application's state: `register_router(state, routes::router)`.
/// Omitting this registration serves every request through PHP.
pub fn register_router<F>(
    state: &AppState,
    build: F,
) -> registry::Result<()>
where
    F: FnOnce(AppState) -> Router,
{
    let router = build(state.clone());
    register_router_with(state, |php| {
        compose(router, move |request: AppRequest| {
            let php = php.clone();
            async move { php.handle(request).await }
        })
    })
}

/// Register a raw service, placing PHP explicitly. Unlike `register_router`,
/// no path or method fallback is added to the result.
///
/// ```ignore
/// lumenize::register_router_with(state, |php| {
///     my_router().fallback_service(php)
/// })
/// ```
pub fn register_router_with<F, S, B>(
    state: &AppState,
    build: F,
) -> registry::Result<()>
where
    F: FnOnce(PhpService) -> S,
    S: Service<AppRequest, Response = Response<B>, Error = Infallible>
        + Clone
        + Send
        + Sync
        + 'static,
    S::Future: Send + 'static,
    B: http_body::Body<Data = claviron::axum::body::Bytes> + Send + 'static,
    B::Error: Into<crate::Error>,
{
    let php = PhpService::new();
    let service = build(php.clone()).map_response(|response| response.map(Body::new));
    state.registry_ref().insert(Arc::new(RustRoutes {
        service: AppService::new(service),
        php,
    }))?;
    Ok(())
}

pub(crate) fn compose<H, T>(
    router: Router,
    fallback: H,
) -> Router
where
    H: Handler<T, ()>,
    T: 'static,
{
    router
        .fallback(fallback.clone())
        .method_not_allowed_fallback(fallback)
}

#[cfg(test)]
#[path = "../tests/routing.rs"]
mod tests;
