use notify::event::{AccessKind, DataChange, ModifyKind};

use super::*;

#[test]
fn reads_and_unselected_files_do_not_restart_php() {
    let roots = vec![PathBuf::from("/app/routes")];
    let event = Event::new(EventKind::Modify(ModifyKind::Data(DataChange::Content)))
        .add_path("/app/routes/web.php".into());
    assert!(relevant(&event, &roots));
    assert!(!relevant(
        &Event::new(EventKind::Access(AccessKind::Read)).add_path("/app/routes/web.php".into()),
        &roots
    ));
    assert!(!relevant(
        &Event::new(EventKind::Create(notify::event::CreateKind::File))
            .add_path("/app/storage/logs/app.log".into()),
        &roots
    ));
}

#[test]
fn directory_moves_and_removals_restart_php() {
    let roots = vec![PathBuf::from("/app/routes")];
    for kind in [
        EventKind::Create(notify::event::CreateKind::Folder),
        EventKind::Remove(notify::event::RemoveKind::Folder),
        EventKind::Modify(ModifyKind::Name(notify::event::RenameMode::Both)),
    ] {
        assert!(relevant(
            &Event::new(kind).add_path("/app/routes/admin".into()),
            &roots
        ));
        assert!(!relevant(
            &Event::new(kind).add_path("/app/storage/cache".into()),
            &roots
        ));
    }
}

#[tokio::test]
async fn notifications_are_retained_coalesced_and_errors_are_not_lost() {
    let changes = Changes::default();
    let roots = vec![PathBuf::from("/app/routes")];
    let event = Event::new(EventKind::Modify(ModifyKind::Data(DataChange::Content)))
        .add_path("/app/routes/web.php".into());
    changes.receive(Ok(event.clone()), &roots);
    changes.receive(Ok(event), &roots);
    changes.changed().await.unwrap();
    changes.receive(Err(notify::Error::generic("watch failed")), &roots);
    changes.changed().await.unwrap_err();
}

#[tokio::test]
async fn native_watch_observes_atomic_file_replacement() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("application.php");
    std::fs::write(&path, "before").unwrap();
    let source = SourceWatch::new(std::slice::from_ref(&path)).await.unwrap();
    let replacement = directory.path().join("replacement.tmp");
    std::fs::write(&replacement, "after").unwrap();
    std::fs::rename(replacement, &path).unwrap();
    source.changes.changed().await.unwrap();
    assert_eq!(std::fs::read_to_string(path).unwrap(), "after");
    source.close().await;
}

#[tokio::test]
async fn watch_waits_for_application_start_then_restarts_only_its_application() {
    use std::sync::atomic::{AtomicBool, Ordering};

    use claviron::app::{
        ApplicationControlResult, ApplicationController, ApplicationLifecycle, ApplicationSnapshot,
    };
    use tokio_util::sync::CancellationToken;

    struct Controller {
        id: ApplicationId,
        ready: AtomicBool,
        checked: Notify,
        restarted: Notify,
    }
    #[async_trait]
    impl ApplicationController for Controller {
        fn runtime(&self) -> &ApplicationRuntime {
            self.id.runtime()
        }

        fn list(&self) -> Vec<ApplicationSnapshot> {
            self.status(self.id.name()).into_iter().collect()
        }

        fn status(
            &self,
            name: &ApplicationName,
        ) -> Option<ApplicationSnapshot> {
            self.checked.notify_one();
            (name == self.id.name() && self.ready.load(Ordering::Acquire)).then(|| {
                ApplicationSnapshot {
                    id: self.id.clone(),
                    lifecycle: ApplicationLifecycle::Running,
                    enabled: true,
                    generation: 1,
                    pool: None,
                    last_error: None,
                }
            })
        }

        async fn start(
            &self,
            _: &ApplicationName,
        ) -> ApplicationControlResult<()> {
            panic!("watch must use restart")
        }

        async fn stop(
            &self,
            _: &ApplicationName,
        ) -> ApplicationControlResult<()> {
            panic!("watch must use restart")
        }

        async fn restart(
            &self,
            name: &ApplicationName,
        ) -> ApplicationControlResult<()> {
            assert_eq!(name, self.id.name());
            self.restarted.notify_one();
            Ok(())
        }
    }
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("application.php");
    std::fs::write(&path, "fixture").unwrap();
    let watcher = Arc::new(SourceWatcher::new("web".into(), vec![path]).unwrap());
    let controller = Arc::new(Controller {
        id: watcher.application.clone(),
        ready: AtomicBool::new(false),
        checked: Notify::new(),
        restarted: Notify::new(),
    });
    let state = AppState::standalone("watch-test", directory.path());
    let manager = Arc::new(ApplicationManager::new());
    manager.add(controller.clone()).unwrap();
    state.registry_ref().insert(manager).unwrap();
    watcher.boot(&state).await.unwrap();
    watcher
        .source
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .changes
        .wake
        .notify_one();
    let stop = CancellationToken::new();
    let run = tokio::spawn(
        watcher
            .clone()
            .run(state.clone(), RunContext::new(stop.clone())),
    );
    controller.checked.notified().await;
    controller.ready.store(true, Ordering::Release);
    state
        .events()
        .emit(claviron::app::management::ApplicationStarted {
            id: watcher.application.clone(),
            generation: 1,
        });
    controller.restarted.notified().await;
    stop.cancel();
    run.await.unwrap().unwrap();
}
