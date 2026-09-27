//! Native filesystem notifications for development-only PHP pool restarts.
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use async_trait::async_trait;
use claviron::app::{ApplicationId, ApplicationManager, ApplicationName, ApplicationRuntime};
use claviron::runtime::registry::{self, ProviderOrder};
use claviron::runtime::{AppState, Provider, RunContext, Runnable};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::Notify;

/// Laravel's source directories; absent defaults are ignored. Custom paths must exist.
pub(crate) fn default_paths() -> Vec<PathBuf> {
    [
        "app",
        "bootstrap",
        "config",
        "php",
        "resources",
        "routes",
        "application.php",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect()
}

#[derive(Default)]
struct Changes {
    wake: Notify,
    error: Mutex<Option<String>>,
}
impl Changes {
    fn receive(
        &self,
        event: notify::Result<Event>,
        paths: &[PathBuf],
    ) {
        match event {
            Ok(event) if relevant(&event, paths) => self.wake.notify_one(),
            Ok(_) => {}
            Err(error) => {
                self.error
                    .lock()
                    .unwrap()
                    .get_or_insert_with(|| error.to_string());
                self.wake.notify_one();
            }
        }
    }

    async fn changed(&self) -> registry::Result<()> {
        self.wake.notified().await;
        match self.error.lock().unwrap().take() {
            Some(error) => Err(registry::Error::msg(format!(
                "source watcher failed: {error}"
            ))),
            None => Ok(()),
        }
    }
}

fn relevant(
    event: &Event,
    paths: &[PathBuf],
) -> bool {
    if matches!(event.kind, EventKind::Access(_)) {
        return false;
    }
    // Moving or removing a directory can change PHP sources without emitting
    // a separate event for each file. Rename events do not retain file type.
    let structural = matches!(
        event.kind,
        EventKind::Create(notify::event::CreateKind::Folder)
            | EventKind::Remove(notify::event::RemoveKind::Folder)
            | EventKind::Modify(notify::event::ModifyKind::Name(_))
    );
    event.need_rescan()
        || event.paths.iter().any(|path| {
            paths.iter().any(|root| path.starts_with(root))
                && (structural
                    || path.extension().is_some_and(|ext| ext == "php")
                    || paths.contains(path))
        })
}

struct Callback {
    changes: Arc<Changes>,
    paths: Vec<PathBuf>,
    closed: Option<tokio::sync::oneshot::Sender<()>>,
}
impl notify::EventHandler for Callback {
    fn handle_event(
        &mut self,
        event: notify::Result<Event>,
    ) {
        self.changes.receive(event, &self.paths);
    }
}
impl Drop for Callback {
    fn drop(&mut self) {
        if let Some(closed) = self.closed.take() {
            let _ = closed.send(());
        }
    }
}

struct SourceWatch {
    watcher: Option<RecommendedWatcher>,
    closed: Option<tokio::sync::oneshot::Receiver<()>>,
    paths: Vec<PathBuf>,
    changes: Arc<Changes>,
}
impl SourceWatch {
    async fn new(paths: &[PathBuf]) -> registry::Result<Self> {
        if matches!(RecommendedWatcher::kind(), notify::WatcherKind::PollWatcher) {
            return Err(registry::Error::msg(
                "watch requires a native filesystem notification backend",
            ));
        }
        let cwd = std::env::current_dir().map_err(failure)?;
        let paths = if paths.is_empty() {
            default_paths()
                .into_iter()
                .filter(|path| path.exists())
                .collect()
        } else {
            paths.to_vec()
        };
        if paths.is_empty() {
            return Err(registry::Error::msg(
                "watch needs a source path; set watch_paths or --watch-path",
            ));
        }
        let paths: Vec<_> = paths
            .into_iter()
            .map(|path| {
                if path.is_absolute() {
                    path
                } else {
                    cwd.join(path)
                }
            })
            .collect();
        for path in &paths {
            std::fs::metadata(path)
                .map_err(|error| failure(format!("watch {}: {error}", path.display())))?;
        }
        let changes = Arc::new(Changes::default());
        let (closed_tx, closed) = tokio::sync::oneshot::channel();
        let callback =
            Callback { changes: changes.clone(), paths: paths.clone(), closed: Some(closed_tx) };
        let watcher =
            RecommendedWatcher::new(callback, notify::Config::default()).map_err(failure)?;
        let mut result = Self { watcher: Some(watcher), closed: Some(closed), paths, changes };
        if let Err(error) = result.attach() {
            result.close().await;
            return Err(error);
        }
        Ok(result)
    }

    async fn close(mut self) {
        drop(self.watcher.take());
        // Notify backends may stop on another thread. Wait until the callback
        // has been released, so no source work survives this generation.
        if let Some(closed) = self.closed.take() {
            let _ = closed.await;
        }
    }

    fn attach(&mut self) -> registry::Result<()> {
        let mut roots = BTreeMap::new();
        for path in &self.paths {
            // Watch the parent too: editors replace files with rename, and a
            // recreated source directory needs its recursive watch restored.
            if let Some(parent) = path.parent() {
                roots.entry(parent.to_owned()).or_insert(false);
            }
            if path.is_dir() {
                roots.insert(path.clone(), true);
            }
        }
        for (path, recursive) in roots {
            if path.exists() {
                self.watcher
                    .as_mut()
                    .expect("watcher is open")
                    .watch(
                        &path,
                        if recursive {
                            RecursiveMode::Recursive
                        } else {
                            RecursiveMode::NonRecursive
                        },
                    )
                    .map_err(failure)?;
            }
        }
        Ok(())
    }
}

pub(crate) struct SourceWatcher {
    application: ApplicationId,
    paths: Vec<PathBuf>,
    manager: OnceLock<Arc<ApplicationManager>>,
    source: Mutex<Option<SourceWatch>>,
}
impl SourceWatcher {
    pub fn new(
        application: String,
        paths: Vec<PathBuf>,
    ) -> registry::Result<Self> {
        Ok(Self {
            application: ApplicationId::new(
                ApplicationRuntime::new("php").map_err(failure)?,
                ApplicationName::new(application).map_err(failure)?,
            ),
            paths,
            manager: OnceLock::new(),
            source: Mutex::new(None),
        })
    }
}
#[async_trait]
impl Provider<AppState> for SourceWatcher {
    fn name(&self) -> &'static str {
        "LumenizeSourceWatcher"
    }

    fn order(&self) -> ProviderOrder {
        ProviderOrder::new().after::<crate::application::Application>()
    }

    async fn boot(
        &self,
        state: &AppState,
    ) -> registry::Result<()> {
        let manager = state
            .registry_ref()
            .resolve::<ApplicationManager>()
            .ok_or_else(|| registry::Error::msg("application manager unavailable"))?;
        self.manager
            .set(manager)
            .map_err(|_| registry::Error::msg("source watcher already booted"))?;
        let source = SourceWatch::new(&self.paths).await?;
        *self.source.lock().unwrap() = Some(source);
        Ok(())
    }

    fn as_finalizable(&self) -> Option<&dyn registry::Finalizable<AppState>> {
        Some(self)
    }

    fn as_runnable(self: Arc<Self>) -> Option<Arc<dyn Runnable<AppState>>> {
        Some(self)
    }
}
#[async_trait]
impl registry::Finalizable<AppState> for SourceWatcher {
    async fn finalize(
        &self,
        _state: &AppState,
    ) -> registry::Result<()> {
        let source = self.source.lock().unwrap().take();
        if let Some(source) = source {
            source.close().await;
        }
        Ok(())
    }
}
#[async_trait]
impl Runnable<AppState> for SourceWatcher {
    async fn run(
        self: Arc<Self>,
        state: AppState,
        context: RunContext,
    ) -> registry::Result<()> {
        let initial = self.source.lock().unwrap().take();
        let mut source = match initial {
            Some(source) => source,
            None => SourceWatch::new(&self.paths).await?,
        };
        let result = async {
        let manager = self
            .manager
            .get()
            .ok_or_else(|| registry::Error::msg("source watcher not booted"))?;
        // Subscribe before checking status so startup cannot race the first edit.
        let mut started = state.events().subscribe::<claviron::app::management::ApplicationStarted>();
        tracing::info!(paths = ?source.paths, "PHP source watch enabled; use development PHP settings with OPcache disabled");
        loop {
            tokio::select! {
                biased;
                () = context.cancelled() => break,
                changed = source.changes.changed() => changed?,
            }
            if context.is_cancelled() {
                break;
            }
            while manager.status(&self.application).is_err() {
                tokio::select! {
                    biased;
                    () = context.cancelled() => return Ok(()),
                    event = started.recv() => match event {
                        Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {},
                        Err(error) => return Err(failure(error)),
                    },
                }
            }
            source.attach()?;
            manager.restart(&self.application).await.map_err(failure)?;
            tracing::info!(application = %self.application, "PHP source changed; application restart requested");
        }
        Ok(())
        }.await;
        source.close().await;
        result
    }
}
fn failure(error: impl std::fmt::Display) -> registry::Error {
    registry::Error::msg(error.to_string())
}

#[cfg(test)]
#[path = "../tests/watch.rs"]
mod tests;
