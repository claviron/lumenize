//! The owner: the launch's PHP application, as the service the runner
//! shell puts behind everything else. Its adapter is bound when PHP has
//! booted, before any ingress runs.
use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};
use std::task::{Context, Poll};

use async_trait::async_trait;
use claviron::ephp::{PhpApplicationAdapter, PhpApplicationService};
use claviron::http::RouteResponse;
use claviron::runtime::registry::{self, Bootable};
use claviron::runtime::{AppState, Component};
use claviron_runner::AppRequest;
use http_body_util::BodyExt;
use tower::Service;

/// The named PHP application as a service, filled at boot.
#[derive(Clone)]
pub(crate) struct PhpService(Arc<OnceLock<PhpApplicationAdapter>>);

impl PhpService {
    pub(crate) fn new() -> Self {
        Self(Arc::new(OnceLock::new()))
    }
}

impl Service<AppRequest> for PhpService {
    type Error = Infallible;
    type Future =
        Pin<Box<dyn Future<Output = Result<RouteResponse, Infallible>> + Send>>;
    type Response = RouteResponse;

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
        let php = Arc::clone(&self.0);
        Box::pin(async move {
            let adapter = php.get().expect("PHP boots before ingress starts");
            adapter.handle(request.map(|body| body.boxed_unsync())).await
        })
    }
}

/// Binds the PHP application's adapter into the owner once PHP has booted,
/// and refuses a launch whose PHP application was not registered.
pub(crate) struct PhpOwner {
    name: String,
    php: PhpService,
}

impl PhpOwner {
    pub(crate) fn new(
        name: String,
        php: PhpService,
    ) -> Self {
        Self { name, php }
    }
}

impl Component<AppState> for PhpOwner {
    /// The PHP application served behind everything else. A master with
    /// workers loads no PHP, so it is required only where the owner binds.
    type Deps = (Option<Arc<PhpApplicationService>>,);

    fn name(&self) -> &'static str {
        "LumenizePhpOwner"
    }

    fn as_bootable(&self) -> Option<&dyn Bootable<AppState, Self::Deps>> {
        Some(self)
    }

    fn validate(
        &self,
        state: &AppState,
    ) -> registry::Result<()> {
        let runtime = state
            .registry_ref()
            .resolve::<claviron::ephp::PhpEmbedRuntime>()
            .ok_or_else(|| {
                registry::Error::msg("PHP runtime was not registered")
            })?;
        if !runtime.has_application(&self.name) {
            return Err(registry::Error::msg(format!(
                "PHP application {:?} is not registered",
                self.name
            )));
        }
        Ok(())
    }
}

#[async_trait]
impl Bootable<AppState, (Option<Arc<PhpApplicationService>>,)> for PhpOwner {
    async fn boot(
        &self,
        (service,): (Option<Arc<PhpApplicationService>>,),
        state: &AppState,
    ) -> registry::Result<()> {
        // A master with workers serves nothing and loads no PHP; its workers
        // each bind their own.
        if state.is_master() {
            return Ok(());
        }
        let service = service.ok_or_else(|| {
            registry::Error::msg("PHP application service was not registered")
        })?;
        let _ = self
            .php
            .0
            .set(PhpApplicationAdapter::new(service, self.name.clone()));
        Ok(())
    }
}
