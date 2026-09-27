use claviron::http::{bytes_body, empty_body};
use http_body_util::BodyExt;

use super::*;

#[tokio::test]
async fn raw_service_works_without_a_router_or_php_boot() {
    let state = AppState::standalone("raw-test", ".");
    register_router_with(&state, |_php| {
        tower::service_fn(|request: AppRequest| async move {
            Ok::<_, Infallible>(Response::new(bytes_body(bytes::Bytes::from(
                request.uri().path().to_owned(),
            ))))
        })
    })
    .unwrap();
    let routes = state.registry_ref().resolve::<RustRoutes>().unwrap();
    let response = routes
        .service
        .clone()
        .oneshot(
            Request::builder()
                .uri("/native")
                .body(body(empty_body()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.into_body().collect().await.unwrap().to_bytes(),
        "/native"
    );
}
