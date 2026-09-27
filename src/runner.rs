use std::sync::Arc;

use claviron::Server;
use claviron::runner::{Owner, launch};
use claviron::runtime::{AppState, registry};

use crate::Options;
use crate::application::{PhpOwner, PhpService};
use crate::options::Launch;

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
        // The application restarts inside its process; with workers each has
        // its own, and the master that watches has none.
        #[cfg(feature = "watch")]
        if self.options.runner.watch && self.options.runner.workers.unwrap_or(0) > 0 {
            return Err(
                "watch restarts the PHP application in its process: run it standalone".into(),
            );
        }
        let config_dir = self.options.config_dir.clone();
        let mut launch = Launch::from(self.options);
        let prepared = launch.prepare(name)?;
        let application = self.application;
        Server::new()
            .name(prepared.name.clone())
            .fleet_name(prepared.name)
            .workers(prepared.workers)
            .launch(prepared.envelope)
            .config_dir(&config_dir)
            .register(move |state| {
                // A worker serves what its master resolved.
                let launch = launch.inherited(state)?;
                let runner = &launch.runner;
                claviron::runner::supervisor::register_policy(
                    state,
                    runner.supervisor.clone().unwrap_or_default(),
                )?;
                claviron::ephp::register(state)?;
                #[cfg(feature = "broadcast")]
                if let Some(broadcaster) = &launch.own.broadcast_application {
                    claviron::laravel::register(state, broadcaster.clone())?;
                }

                // The owner, then what stands in front of it, then the
                // application's own registrations, which compose in front.
                let php = PhpService::new();
                state
                    .registry_ref()
                    .insert(Arc::new(PhpOwner::new(application.clone(), php.clone())))?;
                claviron::runner::register_owner(state, Owner::new(php))?;
                let root = std::env::current_dir()
                    .map_err(|error| registry::Error::msg(error.to_string()))?;
                launch::register_routing(state, runner, &root)?;
                configure(state)?;
                claviron::runner::ensure_application(state)?;
                launch::register_trust(state, runner)?;

                #[cfg(feature = "watch")]
                if runner.watch {
                    let watcher = crate::watch::watcher(
                        root,
                        runner.watch_paths.clone(),
                        application.clone(),
                    )
                    .map_err(registry::Error::msg)?;
                    state.registry_ref().insert(Arc::new(watcher))?;
                }
                // Bounded by the lane's own capacity unless the launch says
                // otherwise: work beyond it waits in the lane, where the edge
                // sees it.
                launch::register_doors(
                    state,
                    runner,
                    PHP_RUNTIME,
                    Some(claviron::orbit::transport::TRANSPORT_INBOX_CAPACITY),
                )
            })
            .serve()
            .await?;
        Ok(())
    }
}
