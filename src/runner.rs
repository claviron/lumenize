use std::sync::Arc;

use claviron::orbit::link::application::LINK_INBOX_CAPACITY;
use claviron::runner::{Composition, Owner};
use claviron::runtime::{AppState, Register, registry};

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
#[derive(Default)]
pub struct Runner {
    options: Options,
    request_mode: Option<crate::PhpRunner>,
    registrations: Vec<Registration>,
    #[cfg(feature = "axum")]
    interceptor: Option<Registration>,
}

impl Runner {
    /// Create a runner whose application name is supplied by its options.
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_options(
        mut self,
        options: Options,
    ) -> Self {
        self.options = options;
        self
    }

    /// Override YAML and automatic detection, independent of builder order.
    pub fn with_request_mode(
        mut self,
        mode: crate::PhpRunner,
    ) -> Self {
        self.request_mode = Some(mode);
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
            claviron::runner::register_router(state, router)
        }));
        self
    }

    pub async fn run(mut self) -> crate::Result<()> {
        let registrations = self.registrations;
        #[cfg(feature = "axum")]
        let interceptor = self.interceptor;
        if let Some(mode) = self.request_mode {
            self.options.configuration.request_mode = Some(mode);
        }
        let root = std::env::current_dir()?;
        let detected_framework =
            self.options.configuration.prepare_php(&root)?;
        if self.options.runner.frameworks.is_none() {
            self.options.runner.frameworks = Some(detected_framework);
        }
        self.options.runner.icon.get_or_insert_with(|| "laravel".into());
        self.options.validate()?;
        let name =
            self.options.runner.name.clone().ok_or("name is required")?;
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
        let config_path = self.options.config_path.clone();
        let launch = Launch::from(self.options);
        let definition_name = name.clone();
        let owner_name = name.clone();
        let application = name.clone();
        // Bounded by the lane's own capacity unless the launch says otherwise:
        // work beyond it waits in the lane, where the edge sees it.
        Composition::new(launch, name, PHP_RUNTIME, move |state, launch| {
            let php = PhpService::new();
            let script =
                if launch.own.request_mode == Some(crate::PhpRunner::Script) {
                    Some(launch.own.script_route(state)?)
                } else {
                    None
                };
            state.registry_ref().register(Arc::new(PhpOwner::new(
                owner_name,
                php.clone(),
                script,
            )))?;
            let owner = Owner::new(php);
            // A Pusher client's channels stand in front of the application.
            #[cfg(feature = "broadcast")]
            let owner = claviron_broadcast::runner::in_front(state, owner)?;
            Ok(owner)
        })
        .default_in_flight(Some(LINK_INBOX_CAPACITY))
        .config_path(&config_path)
        .system_log(system_log)
        .definitions(move |state, launch| {
            launch.own.register(
                state,
                &definition_name,
                launch.runner.frameworks.clone().unwrap_or_default(),
                launch.runner.icon.clone(),
            )?;
            claviron_php::register(state)
        })
        .configure(move |state, _| {
            for register in registrations {
                register(state)?;
            }
            #[cfg(feature = "axum")]
            if let Some(interceptor) = interceptor {
                interceptor(state)?;
            }
            Ok(())
        })
        .settled(move |state, launch| {
            #[cfg(feature = "watch")]
            if launch.runner.watch {
                let root = std::env::current_dir()
                    .map_err(|error| registry::Error::msg(error.to_string()))?;
                let watcher = crate::watch::watcher(
                    root,
                    launch.runner.watch_paths.clone(),
                    application,
                )
                .map_err(registry::Error::msg)?;
                state.registry_ref().register(Arc::new(watcher))?;
            }
            #[cfg(not(feature = "watch"))]
            let _ = (state, launch, application);
            Ok(())
        })
        .serve()
        .await?;
        Ok(())
    }
}
