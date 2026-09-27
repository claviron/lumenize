use std::pin::Pin;
use std::task::{Context, Poll};

use claviron::http::body::BodyLimit;
use claviron::http::bytes_body;
use http_body::{Body, Frame};

use super::*;

#[tokio::test(start_paused = true)]
async fn admission_and_head_expiry_release_capacity() {
    let options: crate::Options = serde_yaml::from_str(
        "max_concurrent_requests: 1\nrequests:\n  queue_timeout: 30s\n  head_timeout: 60s\n",
    )
    .unwrap();
    let app = Application::new(
        "web".into(),
        options.policy(),
        options
            .max_concurrent_requests
            .and_then(|limit| limit.value()),
    );
    let pending = tower::service_fn(|_: crate::routing::AppRequest| {
        std::future::pending::<Result<crate::routing::AppResponse, std::convert::Infallible>>()
    });
    assert!(
        app.router
            .set(crate::routing::AppService::new(pending))
            .is_ok()
    );
    let held = app.admission.enter(None).await.unwrap();
    assert_eq!(
        app.handle(Request::new(empty_body())).await.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    drop(held);
    assert_eq!(
        app.handle(Request::new(empty_body())).await.status(),
        StatusCode::GATEWAY_TIMEOUT
    );
    // The timed-out handler must leave its place available for the next request.
    assert!(
        app.admission
            .enter(options.policy().requests.queue_timeout)
            .await
            .is_some()
    );
}

#[tokio::test(start_paused = true)]
async fn response_head_releases_capacity_before_body_finishes() {
    let options: crate::Options =
        serde_yaml::from_str("requests:\n  queue_timeout: 30s\n").unwrap();
    let app = Application::new("web".into(), options.policy(), Some(1));
    let streaming = tower::service_fn(|_: crate::routing::AppRequest| async {
        Ok::<_, std::convert::Infallible>(Response::new(crate::routing::body(Stalled)))
    });
    assert!(
        app.router
            .set(crate::routing::AppService::new(streaming))
            .is_ok()
    );
    let first = app.handle(Request::new(empty_body())).await;
    assert_eq!(first.status(), StatusCode::OK);
    let second = app.handle(Request::new(empty_body())).await;
    assert_eq!(second.status(), StatusCode::OK);
    drop((first, second));
}

fn app() -> Application {
    let mut policy = HttpPolicy::unlimited();
    policy.requests.max_body_size = Some(4);
    Application::new("web".into(), policy, None)
}

#[tokio::test]
async fn request_guard_rejects_declared_and_undeclared_oversize_bodies() {
    let declared = Request::builder()
        .header("content-length", "5")
        .body(empty_body())
        .unwrap();
    assert_eq!(
        app().prepare(declared).unwrap_err(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
    let streamed = Request::new(bytes_body(vec![b'x'; 5].into()));
    let error = app()
        .prepare(streamed)
        .unwrap()
        .into_body()
        .collect()
        .await
        .unwrap_err();
    assert_eq!(
        error.downcast_ref::<BodyLimit>(),
        Some(&BodyLimit::TooLarge { limit: 4 })
    );
}

struct Stalled;
impl Body for Stalled {
    type Data = <RouteBody as Body>::Data;
    type Error = std::io::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        Poll::Pending
    }
}

#[tokio::test(start_paused = true)]
async fn request_guard_enforces_the_configured_read_gap() {
    // The fixture's declared policy is advanced virtually, never slept out.
    let options: crate::Options =
        serde_yaml::from_str("requests:\n  body_read_timeout: 30s\n").unwrap();
    let policy = options.policy();
    let app = Application::new("web".into(), policy, None);
    let request = Request::new(claviron::http::boxed_body(Stalled));
    let error = app
        .prepare(request)
        .unwrap()
        .into_body()
        .collect()
        .await
        .unwrap_err();
    assert_eq!(
        error.downcast_ref::<BodyLimit>(),
        Some(&BodyLimit::Stalled { after: policy.requests.body_read_timeout.unwrap() })
    );
}

#[tokio::test(start_paused = true)]
async fn response_guard_enforces_the_configured_limit_and_gap() {
    let options: crate::Options =
        serde_yaml::from_str("responses:\n  max_body_size: 4\n  body_idle_timeout: 60s\n").unwrap();
    let policy = options.policy();
    let app = Application::new("web".into(), policy, None);
    let response = Response::new(bytes_body(vec![b'x'; 5].into()));
    let error = app
        .finish(response)
        .into_body()
        .collect()
        .await
        .unwrap_err();
    assert_eq!(
        error.downcast_ref::<BodyLimit>(),
        Some(&BodyLimit::TooLarge { limit: 4 })
    );
    let response = Response::new(claviron::http::boxed_body(Stalled));
    let error = app
        .finish(response)
        .into_body()
        .collect()
        .await
        .unwrap_err();
    assert_eq!(
        error.downcast_ref::<BodyLimit>(),
        Some(&BodyLimit::Stalled { after: policy.responses.body_idle_timeout.unwrap() })
    );
}
