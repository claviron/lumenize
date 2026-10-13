use std::sync::Arc;

use claviron::Server;
use claviron::runtime::{AppState, Register, registry};
use claviron_runner::{Owner, launch};

use crate::Options;
use crate::application::{PhpOwner, PhpService};
use crate::options::Launch;

/// What the Orbit lane says answers the name: a Python application given the
/// same name is refused at its join rather than splitting this one's requests.
const PHP_RUNTIME: &str = "php";

type Registration = Box<dyn FnOnce(&AppState) -> registry::Result<()> + Send>;

/// Compose one application from the launch file on Claviron's runner shell: its
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

    /// Read one composition file. Paths inside it remain relative to cwd.
    pub fn with_config_path(
        mut self,
        path: impl AsRef<std::path::Path>,
    ) -> crate::Result<Self> {
        self.options = Options::from_file(path)?;
        Ok(self)
    }

    /// Select Orbit's namespace independently of this application's name.
    pub fn with_fleet_name(
        mut self,
        name: impl Into<crate::FleetName>,
    ) -> Self {
        self.options.runner.fleet_name = Some(name.into().to_string());
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

    pub async fn run(mut self) -> crate::Result<()> {
        let registrations = self.registrations;
        #[cfg(feature = "axum")]
        let interceptor = self.interceptor;
        if self.options.runner.frameworks.is_none() {
            let root = std::env::current_dir()?;
            self.options.runner.frameworks = Some(
                claviron::ephp::framework::ComposerPackages::read(&root)
                    .map(|packages| packages.frameworks())
                    .unwrap_or_default(),
            );
        }
        self.options.runner.icon.get_or_insert_with(|| "laravel".into());
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
        let system_log = self.options.system_log()?;
        let production = self.options.runner.production;
        let environment = self.options.runner.environment.clone();
        let mut launch = Launch::from(self.options);
        let prepared = launch.prepare(name)?;
        let application = self.application;
        let definitions = launch.clone();
        let definition_name = application.clone();
        // Production only by `--production`; otherwise `--environment`, and
        // without either, `local`.
        let server = match (production, environment) {
            (true, _) => Server::new().production(),
            (false, Some(environment)) => {
                Server::new().environment(environment)
            }
            (false, None) => Server::new(),
        };
        server
            .name(prepared.name.clone())
            .fleet_name(prepared.fleet_name)
            .workers(prepared.workers)
            .launch(prepared.envelope)
            .config_dir(".")
            .system_log(system_log)
            .with_definitions(move |state| {
                let definitions = definitions.inherited(state)?;
                claviron_runner::supervisor::register_policy(
                    state,
                    definitions.runner.supervisor.clone().unwrap_or_default(),
                )?;
                definitions.own.register(
                    state,
                    &definition_name,
                    definitions.runner.frameworks.clone().unwrap_or_default(),
                    definitions.runner.icon.clone(),
                )
            })
            .register(move |state| {
                // A worker serves what its master resolved.
                let launch = launch.inherited(state)?;
                let runner = &launch.runner;

                // The owner, then what stands in front of it, then the
                // application's own registrations, which compose in front.
                let php = PhpService::new();
                state.registry_ref().register(Arc::new(PhpOwner::new(
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
                    state.registry_ref().register(Arc::new(watcher))?;
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
