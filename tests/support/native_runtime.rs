// Kept package-local so the published Laravel crate can run its native test
// without reaching into another package's test source.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use claviron::runtime::registry::Runnable;
use claviron::runtime::{AppState, RunContext, RunMode};
use claviron_php::embed::{
    PhpEmbedRuntime, PhpEmbedStateExt, PhpSapiRequest, PhpScriptService,
    RequestPool,
};
use claviron_php::{PhpRunner, PhpRuntime};
use http::Request;
use http_body_util::BodyExt;
use tokio_util::sync::CancellationToken;

/// A path as a single-quoted PHP string literal.
pub fn php_string(path: &Path) -> String {
    let path = path.to_str().unwrap();
    format!("'{}'", path.replace('\\', "\\\\").replace('\'', "\\'"))
}

/// One runtime per integration-test process, with no listener or fleet boot.
///
/// `prepare` registers what the fixture needs, such as an extension or a
/// cache, before PHP boots. The fixture runs from its own directory, so it
/// can reach this package's files, and answers with `X-Fixture: ok` and the
/// body `ok`.
pub async fn run_fixture(
    fixture: &Path,
    prepare: impl AsyncFnOnce(&AppState),
) {
    let root = tempfile::tempdir().unwrap();
    let ini = root.path().join("php.ini");
    std::fs::write(
        &ini,
        "max_execution_time=0\ndisplay_errors=0\nlog_errors=1\n",
    )
    .unwrap();
    let script = root.path().join("index.php");
    std::fs::write(
        &script,
        format!("<?php require {};\n", php_string(fixture)),
    )
    .unwrap();
    let state = AppState::standalone("php-extension-test", root.path());
    prepare(&state).await;
    PhpEmbedRuntime::builder()
        .request_pool(RequestPool::fixed(2))
        .cache_invalidation(false)
        .ini_path(ini)
        .ini_scan_dir(root.path())
        .build()
        .register(&state)
        .unwrap();
    let php = state.php_embed().unwrap();
    claviron::runtime::AnyComponent::boot(
        php.as_ref(),
        state.registry_ref(),
        &state,
    )
    .await
    .unwrap();
    let service = state.registry_ref().resolve::<PhpScriptService>().unwrap();
    let cancellation = CancellationToken::new();
    let generation = service.clone().run(
        php.clone(),
        state,
        RunContext::new(cancellation.clone()),
    );
    tokio::pin!(generation);
    // The first poll synchronously starts the pool, then parks on cancellation.
    assert!(futures_util::poll!(&mut generation).is_pending());
    let runtime = Arc::new(PhpRuntime {
        mapping: None,
        path: None,
        entry_script: None,
        entry_query: None,
        try_files: false,
        entry_extensions: Arc::from([]),
        validate_script: false,
        indexes: Arc::from(["index.php".to_owned()]),
        dir: root.path().to_path_buf(),
        fpm: Arc::from([]),
        environment: Arc::new(HashMap::new()),
        env_file: Arc::from([]),
        execution_environment: Default::default(),
        php_runner: PhpRunner::Script,
        application: None,
        server_run_mode: RunMode::Standalone,
        server_worker_count: 1,
        server_pid: std::process::id(),
        server_hostname: "app.test".to_owned(),
        server_software: "claviron-test".to_owned(),
    });
    // Both ZTS threads and a reused request context must retain the metadata.
    for _ in 0..3 {
        let request = PhpSapiRequest {
            script_name: "/index.php".to_owned(),
            script_file: script.clone(),
            document_uri: "/index.php".to_owned(),
            request_uri: "/index.php".to_owned(),
            query_string: String::new(),
            path_info: None,
        };
        let response = service
            .execute(
                runtime.clone(),
                request,
                Request::builder()
                    .extension(claviron::http::policy::HttpPolicy::unlimited())
                    .body(claviron::http::empty_body())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (parts, body) = response.into_parts();
        let body = body.collect().await.unwrap().to_bytes();
        assert_eq!(
            parts.status,
            http::StatusCode::OK,
            "{}",
            String::from_utf8_lossy(&body)
        );
        assert_eq!(parts.headers["x-fixture"], "ok");
        assert_eq!(body.as_ref(), b"ok");
    }
    cancellation.cancel();
    generation.await.unwrap();
    assert_eq!(php.pool_snapshot().unwrap().threads_current, 0);
}
