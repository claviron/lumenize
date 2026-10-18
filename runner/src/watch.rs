//! Development's reload for PHP. The watching is the runner shell's
//! (`claviron::runner::watch`); what is Lumenize's is what counts as source
//! and what a change does: the PHP application restarts in this process,
//! draining its own generation, once it has started.
use std::path::PathBuf;
use std::sync::Arc;

use claviron::app::management::ApplicationStarted;
use claviron::app::{
    ApplicationId, ApplicationManager, ApplicationName, ApplicationRuntime,
};
use claviron::runner::watch::{Reaction, Source, SourceWatcher};

/// Laravel's source; with no paths written, its source directories and the
/// application entry, those that exist.
const LARAVEL: Source = Source {
    extensions: &["php"],
    skip: &["vendor", "storage", "node_modules", "public", ".git", "target"],
    defaults: &[
        "app",
        "bootstrap",
        "config",
        "php",
        "resources",
        "routes",
        "application.php",
    ],
};

pub(crate) fn watcher(
    root: PathBuf,
    paths: Vec<PathBuf>,
    application: String,
) -> Result<SourceWatcher, String> {
    let application = ApplicationId::new(
        ApplicationRuntime::new("php").map_err(|error| error.to_string())?,
        ApplicationName::new(application).map_err(|error| error.to_string())?,
    );
    let restart: Reaction = Arc::new(move |state| {
        let application = application.clone();
        Box::pin(async move {
            let manager = state
                .registry_ref()
                .resolve::<ApplicationManager>()
                .ok_or_else(|| {
                    "the application manager is not registered".to_owned()
                })?;
            // Subscribed before asking, so a start between the two is seen.
            let mut started = state.events().subscribe::<ApplicationStarted>();
            while manager.status(&application).is_err() {
                match started.recv().await {
                    Ok(_)
                    | Err(tokio::sync::broadcast::error::RecvError::Lagged(
                        _,
                    )) => {}
                    Err(error) => return Err(error.to_string()),
                }
            }
            manager
                .restart(&application)
                .await
                .map_err(|error| error.to_string())?;
            Ok(format!("PHP application {}", application.name()))
        })
    });
    Ok(SourceWatcher::new(root, paths, LARAVEL, restart))
}
