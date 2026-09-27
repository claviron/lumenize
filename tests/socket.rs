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
    let error = serve(server, None, app, stop, None, None)
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), "Cancelled");
}

#[cfg(feature = "tls")]
#[tokio::test]
async fn cancellation_ends_a_pending_tls_handshake() {
    let fixture = crate::tls::tests::Fixture::new();
    let acceptor = fixture.tls.acceptor().unwrap();
    let (server, _silent_client) = tokio::io::duplex(1);
    let stop = CancellationToken::new();
    let owner = SessionOwner::new("test".parse().unwrap());
    let gate = claviron::runtime::Gate::new(None, std::time::Duration::ZERO);
    let scope = owner.bind(gate.try_enter().unwrap());
    let app = Arc::new(Application::new(
        "web".into(),
        claviron::http::policy::HttpPolicy::unlimited(),
        None,
    ));
    let address = SocketAddr::from(([127, 0, 0, 1], 0));
    let connection = serve_tls(
        server,
        (address, address),
        app,
        stop.clone(),
        scope,
        acceptor,
    );
    tokio::pin!(connection);
    std::future::poll_fn(|cx| {
        use std::future::Future;
        assert!(connection.as_mut().poll(cx).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    stop.cancel();
    connection.await.unwrap();
    owner.close().join().await;
}

#[cfg(feature = "tls")]
#[test]
fn physical_tls_sets_https_without_forwarded_headers() {
    use claviron::http::ext::ClientAuthStatus;
    let context = connection_context(
        None,
        Some(TlsInfo {
            protocol_version: None,
            cipher_suite: None,
            client_auth: ClientAuthStatus::Verified,
            client_cert: None,
        }),
    );
    assert_eq!(context.scheme(), http::uri::Scheme::HTTPS);
    assert_eq!(context.tls().unwrap().ssl_client_verify(), "SUCCESS");
    assert_eq!(
        connection_context(None, None).scheme(),
        http::uri::Scheme::HTTP
    );
}
