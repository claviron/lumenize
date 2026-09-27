use super::*;

#[test]
fn cleanup_removes_only_the_socket_path_this_generation_owned() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.sock");
    std::fs::write(&path, b"old").unwrap();
    let guard = SocketFile::owned(path.clone()).unwrap();
    std::fs::rename(&path, dir.path().join("old.sock")).unwrap();
    std::fs::write(&path, b"replacement").unwrap();
    drop(guard);
    assert_eq!(std::fs::read(&path).unwrap(), b"replacement");
    drop(SocketFile::owned(path.clone()).unwrap());
    assert!(!path.exists());
}

#[tokio::test]
async fn cancellation_ends_an_idle_connection_before_protocol_detection() {
    let (server, _client) = tokio::io::duplex(1);
    let stop = CancellationToken::new();
    stop.cancel();
    let app = Arc::new(Application::new(
        "web".into(),
        claviron::http::policy::HttpPolicy::unlimited(),
        None,
    ));
    // Hyper reports cancellation while still detecting the protocol as an
    // error. Completion is the contract: it must not wait for client bytes.
    let error = serve(server, None, app, stop, None).await.unwrap_err();
    assert_eq!(error.to_string(), "Cancelled");
}
