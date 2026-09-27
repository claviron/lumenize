use http::Request;
use http_body_util::BodyExt;
use tower::ServiceExt;

use super::*;

#[tokio::test]
async fn mounts_serve_assets_and_preserve_application_fallback_and_boundaries() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("build")).unwrap();
    std::fs::write(dir.path().join("build/app.css"), "body{}").unwrap();
    std::fs::write(dir.path().join("build/secret.php"), "private PHP source").unwrap();
    std::fs::write(dir.path().join("build/.env"), "private environment").unwrap();
    let files = Arc::new(Files {
        root: dir.path().to_owned(),
        statics: vec![StaticConfig {
            path: "/build".into(),
            dir: "build".into(),
            none_found: false,
            rules: None,
        }],
    });
    let fallback = AppService::new(tower::service_fn(|_: AppRequest| async {
        Ok::<_, std::convert::Infallible>(http::Response::new(Body::from("application")))
    }));
    let service = wrap(Some(files), fallback);
    for (method, path, status, body) in [
        (Method::GET, "/build/app.css", StatusCode::OK, "body{}"),
        (Method::HEAD, "/build/app.css", StatusCode::OK, ""),
        (
            Method::POST,
            "/build/app.css",
            StatusCode::OK,
            "application",
        ),
        (Method::GET, "/runtime", StatusCode::OK, "application"),
        // Dynamic files go to the application; their source is never served.
        (
            Method::GET,
            "/build/secret.php",
            StatusCode::OK,
            "application",
        ),
        (
            Method::GET,
            "/builder/app.css",
            StatusCode::OK,
            "application",
        ),
    ] {
        let response = service
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
        assert_eq!(response.status(), status, "{path}");
        assert_eq!(
            response.into_body().collect().await.unwrap().to_bytes(),
            body,
            "{path}"
        );
    }
    for path in [
        "/build/.env",
        "/build/%2e%2e/secret.php",
        "/build/missing.css",
    ] {
        let response = service
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_ne!(response.status(), StatusCode::OK, "{path}");
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert!(!String::from_utf8_lossy(&body).contains("private"));
    }
}
