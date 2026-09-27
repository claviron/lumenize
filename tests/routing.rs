use claviron::axum::body::Body;
use claviron::axum::http::{Request, StatusCode};
use claviron::axum::routing::get;
use http_body_util::BodyExt;
use tower::ServiceExt;

use super::*;

#[tokio::test]
async fn builders_register_without_booting_php_and_raw_preserves_dispatch() {
    let state = AppState::standalone("routing-test", ".");
    register_router(&state, |_| {
        Router::new().route("/native", get(|| async { "rust" }))
    })
    .unwrap();
    let routes = state.registry_ref().resolve::<RustRoutes>().unwrap();
    let response = routes
        .service
        .clone()
        .oneshot(
            Request::builder()
                .uri("/native")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.into_body().collect().await.unwrap().to_bytes(),
        "rust"
    );
    assert!(register_router(&state, |_| Router::new()).is_err());

    let state = AppState::standalone("raw-routing-test", ".");
    register_router_with(&state, |_php| {
        Router::new()
            .route("/native", get(|| async { "rust" }))
            .fallback(|| async { StatusCode::IM_A_TEAPOT })
    })
    .unwrap();
    let routes = state.registry_ref().resolve::<RustRoutes>().unwrap();
    for (method, path, status) in [
        ("GET", "/native", StatusCode::OK),
        ("POST", "/native", StatusCode::METHOD_NOT_ALLOWED),
        ("GET", "/missing", StatusCode::IM_A_TEAPOT),
    ] {
        let response = routes
            .service
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), status);
    }
}

#[tokio::test]
async fn only_undeclared_paths_and_methods_fall_back_to_php() {
    let router = compose(
        Router::new()
            .route("/native", get(|| async { "rust" }))
            .route("/native-missing", get(|| async { StatusCode::NOT_FOUND })),
        || async { "php" },
    );
    for (method, path, status, body) in [
        ("GET", "/native", StatusCode::OK, "rust"),
        ("POST", "/native", StatusCode::OK, "php"),
        ("GET", "/laravel", StatusCode::OK, "php"),
        ("GET", "/native-missing", StatusCode::NOT_FOUND, ""),
    ] {
        let request = Request::builder()
            .method(method)
            .uri(path)
            .body(Body::empty())
            .unwrap();
        let response = router.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), status);
        assert_eq!(
            response.into_body().collect().await.unwrap().to_bytes(),
            body
        );
    }
}
