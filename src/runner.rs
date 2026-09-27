use std::sync::Arc;

use claviron::Server;
use claviron::runtime::{AppState, registry};

use crate::Options;
use crate::application::Application;

/// Compose one named php.yaml application with socket and/or Orbit ingress.
/// Claviron owns the PHP engine, services, signals, draining and finalization.
pub struct Runner {
    application: String,
    options: Options,
}
impl Runner {
    pub fn new(application: impl Into<String>) -> Self {
        Self { application: application.into(), options: Options::default() }
    }

    pub fn with_options(
        mut self,
        options: Options,
    ) -> Self {
        self.options = options;
        self
    }

    pub async fn run<F>(
        self,
        configure: F,
    ) -> crate::Result<()>
    where
        F: FnOnce(&AppState) -> registry::Result<()> + Send + 'static,
    {
        self.options.validate()?;
        let name = self
            .options
            .name
            .clone()
            .unwrap_or_else(|| self.application.clone());
        claviron::orbit::transport::segment::check_application_name(&name)?;
        let options = self.options;
        Server::new()
            .name(name.clone())
            .fleet_name(name.clone())
            .workers(0)
            .config_dir(&options.config_dir)
            .register(move |state| {
                claviron::ephp::register(state)?;
                #[cfg(feature = "broadcast")]
                if let Some(application) = &options.broadcast_application {
                    claviron::laravel::register(state, application.clone())?;
                }
                configure(state)?;
                #[cfg(feature = "watch")]
                if options.watch {
                    state
                        .registry_ref()
                        .insert(Arc::new(crate::watch::SourceWatcher::new(
                            self.application.clone(),
                            options.watch_paths.clone(),
                        )?))?;
                }
                let application = Application::new(
                    self.application,
                    options.policy(),
                    options
                        .max_concurrent_requests
                        .and_then(|limit| limit.value()),
                );
                #[cfg(feature = "files")]
                let application = application.with_files(options.files.clone());
                state.registry_ref().insert(Arc::new(application))?;
                if options.tcp.is_some() || options.uds.is_some() {
                    state
                        .registry_ref()
                        .insert(Arc::new(crate::socket::SocketIngress::new(
                            options.tcp,
                            options.uds.clone(),
                            #[cfg(feature = "tls")]
                            options.tls_cert.as_ref().zip(options.tls_key.as_ref()).map(
                                |(certificate, key)| crate::Tls {
                                    certificate: certificate.clone(),
                                    key: key.clone(),
                                    client_ca: options.tls_client_ca.clone(),
                                },
                            ),
                        )))?;
                }
                if options.orbit {
                    state
                        .registry_ref()
                        .insert(Arc::new(crate::orbit::OrbitIngress::new(
                            name,
                            options.max_in_flight,
                        )))?;
                }
                Ok(())
            })
            .serve()
            .await?;
        Ok(())
    }
}
