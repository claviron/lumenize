use std::path::PathBuf;
use std::sync::Arc;

use claviron::Server;
use claviron::runtime::{AppState, Register, registry};
use claviron_runner::{Owner, launch};

use crate::application::{PhpOwner, PhpService};
use crate::options::Launch;
use crate::{FleetName, Options};

/// What the Orbit lane says answers the name: a Python application given the
/// same name is refused at its join rather than splitting this one's requests.
const PHP_RUNTIME: &str = "php";

type Registration = Box<dyn FnOnce(&AppState) -> registry::Result<()> + Send>;

/// Compose one named php.yaml application on Claviron's runner shell: its
/// sockets, its Orbit lane, its routing, its workers. Claviron owns the PHP
/// engine, services, signals, draining and finalization.
pub struct Runner {
    application: String,
    options: Options,
    registrations: Vec<Registration>,
    #[cfg(feature = "axum")]
    interceptor: Option<Registration>,
}

impl Runner {
    pub fn new(application: impl Into<String>) -> Self {
        Self {
            application: application.into(),
            options: Options::default(),
            registrations: Vec::new(),
            #[cfg(feature = "axum")]
            interceptor: None,
        }
    }

    pub fn with_options(
        mut self,
        options: Options,
    ) -> Self {
        self.options = options;
        self
    }

    /// Where runtime configuration is read; relative paths use the process cwd.
    pub fn with_config_dir(
        mut self,
        directory: impl Into<PathBuf>,
    ) -> Self {
        self.options.config_dir = directory.into();
        self
    }

    /// Register application services by type, without constructing or booting
    /// them in the builder. Tuples register in declaration order.
    pub fn with_services<C: Register<AppState> + 'static>(self) -> Self {
        self.with_bootstrap(C::register)
    }
    /// Add application-owned registrations before provider boot.
    pub fn with_bootstrap<F>(
        mut self,
        configure: F,
    ) -> Self
    where
        F: FnOnce(&AppState) -> registry::Result<()> + Send + 'static,
    {
        self.registrations.push(Box::new(configure));
        self
    }

    /// Rust handles its declared routes; unmatched paths and methods go to PHP.
    /// Build after application registrations so their Routable contributions
    /// are available. Calling this again replaces the selected interceptor.
    #[cfg(feature = "axum")]
    pub fn with_interceptor<F>(
        mut self,
        router: F,
    ) -> Self
    where
        F: FnOnce(AppState) -> claviron::axum::Router + Send + 'static,
    {
        self.interceptor = Some(Box::new(move |state| {
            claviron_runner::register_router(state, router)
        }));
        self
    }

    pub fn with_orbit_announce(
        mut self,
        fleet_name: FleetName,
    ) -> Self {
        self.options.edge_fleet = Some(fleet_name);
        self
    }

    /// Select the configured broadcast application for Laravel's native
    /// publisher. Its credentials and endpoints stay in broadcast.yaml.
    #[cfg(feature = "broadcast")]
    pub fn with_broadcasting(
        mut self,
        application: impl Into<String>,
    ) -> Self {
        self.options.broadcast_application = Some(application.into());
        self
    }

    pub async fn run<F>(mut self) -> crate::Result<()>
where {
        let registrations = self.registrations;
        #[cfg(feature = "axum")]
        let interceptor = self.interceptor;
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
        if self.options.runner.watch
            && self.options.runner.workers.unwrap_or(0) > 0
        {
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
                claviron_runner::supervisor::register_policy(
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
                state.registry_ref().insert(Arc::new(PhpOwner::new(
                    application.clone(),
                    php.clone(),
                )))?;
                claviron_runner::register_owner(state, Owner::new(php))?;
                let root = std::env::current_dir()
                    .map_err(|error| registry::Error::msg(error.to_string()))?;
                launch::register_routing(state, runner, &root)?;
                for register in registrations {
                    register(state)?;
                }
                #[cfg(feature = "axum")]
                if let Some(interceptor) = interceptor {
                    interceptor(state)?;
                }
                claviron_runner::ensure_application(state)?;
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
