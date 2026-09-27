use claviron::http::empty_body;
use claviron::http::session::SessionOwner;
use claviron::runtime::Gate;
use http::{Method, Request, Version};

use super::*;

#[tokio::test]
async fn upgrades_require_a_registered_path_and_native_session_owner() {
    let broadcast = Broadcast {
        paths: HashSet::from(["/app/demo-key".to_owned()]),
        service: Arc::new(BroadcastService::new(
            serde_yaml::from_str("applications: {}").unwrap(),
        )),
    };
    let mut request = Request::builder()
        .uri("/app/demo-key")
        .body(empty_body())
        .unwrap();
    assert!(!broadcast.accepts(&request)); // Orbit HTTP carries no upgrade owner.
    let owner = SessionOwner::new("test".parse().unwrap());
    let gate = Gate::new(None, std::time::Duration::ZERO);
    let scope = owner.bind(gate.try_enter().unwrap());
    request.extensions_mut().insert(scope.sessions());
    assert!(broadcast.accepts(&request));
    *request.uri_mut() = "/app/unknown".parse().unwrap();
    assert!(!broadcast.accepts(&request));
    *request.uri_mut() = "/app/demo-key".parse().unwrap();
    *request.version_mut() = Version::HTTP_2;
    assert!(!broadcast.accepts(&request));
    *request.version_mut() = Version::HTTP_11;
    *request.method_mut() = Method::POST;
    assert!(!broadcast.accepts(&request));
    owner.close().join().await;
}
