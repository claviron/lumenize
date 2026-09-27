use std::sync::Arc;

use claviron::Server;
use claviron::runner::{OrbitIngress, Owner, Routing, SocketIngress};
use claviron::runtime::{AppState, registry};

use crate::Options;
use crate::application::{PhpOwner, PhpService};

/// What the Orbit lane says answers the name: a Python application given the
/// same name is refused at its join rather than splitting this one's requests.
const PHP_RUNTIME: &str = "php";

/// Compose one named php.yaml application on Claviron's runner shell: its
/// sockets, its Orbit lane, its routing, its workers. Claviron owns the PHP
/// engine, services, signals, draining and finalization.
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
            .runner
            .name
            .clone()
            .unwrap_or_else(|| self.application.clone());
        claviron::orbit::transport::segment::check_application_name(&name)?;
        let workers = self.options.runner.workers.unwrap_or(0);
        #[cfg(feature = "watch")]
        if self.options.runner.watch && workers > 0 {
            // The application restarts inside its process; with workers each
            // has its own, and the master that watches has none.
            return Err(
                "watch restarts the PHP application in its process: run it standalone".into(),
            );
        }
        // A worker is started with no command line; what the master resolved
        // travels to it in the launch.
        let launch = self.options.encode()?;
        let resolved = self.options;
        let application = self.application;
        Server::new()
            .name(name.clone())
            .fleet_name(name.clone())
            .workers(workers)
            .launch(launch)
            .config_dir(&resolved.config_dir)
            .register(move |state| {
                let options = if state.is_worker() {
                    state
                        .registry_ref()
                        .resolve::<claviron::runtime::LaunchBlob>()
                        .filter(|blob| !blob.is_empty())
                        .map(|blob| Options::decode(blob.as_bytes()))
                        .transpose()
                        .map_err(|error| registry::Error::msg(error.to_string()))?
                        .unwrap_or(resolved)
                } else {
                    resolved
                };
                let runner = &options.runner;
                // The name the master resolved: a worker's own command line is
                // empty, so what it read from the file is not what was asked.
                let name = runner.name.clone().unwrap_or_else(|| application.clone());
                claviron::runner::supervisor::register_policy(
                    state,
                    runner.supervisor.clone().unwrap_or_default(),
                )?;
                claviron::ephp::register(state)?;
                #[cfg(feature = "broadcast")]
                if let Some(broadcaster) = &options.broadcast_application {
                    claviron::laravel::register(state, broadcaster.clone())?;
                }

                // The owner first, then the application's own registrations,
                // which compose in front of it.
                let php = PhpService::new();
                state
                    .registry_ref()
                    .insert(Arc::new(PhpOwner::new(application.clone(), php.clone())))?;
                claviron::runner::register_owner(state, Owner::new(php))?;
                let policy = runner.policy().map_err(registry::Error::msg)?;
                let routing = Routing::new(policy, runner.max_concurrent_requests);
                #[cfg(feature = "files")]
                let routing = routing.files(
                    runner
                        .files
                        .as_ref()
                        .map(|files| files.parse().map(Arc::new))
                        .transpose()
                        .map_err(registry::Error::msg)?,
                );
                state.registry_ref().insert(Arc::new(routing))?;
                configure(state)?;
                claviron::runner::ensure_application(state)?;

                #[cfg(feature = "watch")]
                if runner.watch {
                    let root = std::env::current_dir()
                        .map_err(|error| registry::Error::msg(error.to_string()))?;
                    let watcher = crate::watch::watcher(
                        root,
                        runner.watch_paths.clone(),
                        application.clone(),
                    )
                    .map_err(registry::Error::msg)?;
                    state.registry_ref().insert(Arc::new(watcher))?;
                }
                let binds = runner.binds();
                if !binds.is_empty() {
                    let ingress = SocketIngress::new(binds, runner.protocols());
                    #[cfg(feature = "tls")]
                    let ingress = ingress.tls(runner.tls());
                    state.registry_ref().insert(Arc::new(ingress))?;
                }
                if runner.orbit {
                    // Bounded by the lane's own capacity unless the launch
                    // says otherwise: work beyond it waits in the lane, where
                    // the edge sees it.
                    let max_in_flight = runner
                        .max_in_flight
                        .unwrap_or(claviron::orbit::transport::TRANSPORT_INBOX_CAPACITY);
                    state.registry_ref().insert(Arc::new(OrbitIngress::new(
                        name,
                        PHP_RUNTIME,
                        Some(max_in_flight),
                    )))?;
                }
                Ok(())
            })
            .serve()
            .await?;
        Ok(())
    }
}
