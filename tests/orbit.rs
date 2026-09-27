use std::pin::Pin;
use std::sync::Mutex;
use std::task::{Context, Poll};

use claviron::http::bytes_body;
use tokio::io::AsyncWrite;

use super::*;

#[test]
fn original_uri_and_peer_metadata_survive_orbit() {
    let exchange = Exchange {
        method: "GET".into(),
        scheme: "https".into(),
        raw_path: b"/a%2Fb".to_vec(),
        path: "/a/b".into(),
        query: b"x=1&x=2".to_vec(),
        headers: vec![
            (b"host".to_vec(), b"app.test".to_vec()),
            (b"x-repeat".to_vec(), b"one".to_vec()),
            (b"x-repeat".to_vec(), b"two".to_vec()),
        ],
        client: Some("127.0.0.2:1234".parse().unwrap()),
        server: Some("127.0.0.1:443".parse().unwrap()),
        ..Exchange::default()
    };
    let request = build_request(&exchange, empty_body()).unwrap();
    assert_eq!(request.uri(), "/a%2Fb?x=1&x=2");
    assert_eq!(request.headers().get_all("x-repeat").iter().count(), 2);
    let conn = request.extensions().get::<Arc<ConnCtx>>().unwrap();
    assert_eq!(conn.remote, exchange.client.unwrap());
    assert!(conn.tls.is_some());
}

#[test]
fn request_reads_fit_the_declared_body_and_window() {
    for (length, expected) in [
        (0, 1),
        (12345, 12345),
        (TRANSPORT_BUFFER_BYTES * 2, TRANSPORT_BUFFER_BYTES),
    ] {
        let exchange = Exchange {
            headers: vec![(b"Content-Length".to_vec(), length.to_string().into_bytes())],
            ..Exchange::default()
        };
        assert_eq!(request_piece(&exchange), expected);
    }
    assert_eq!(request_piece(&Exchange::default()), TRANSPORT_BUFFER_BYTES);
}

#[derive(Default)]
struct Writes {
    bytes: usize,
    closed: bool,
    head: bool,
}
struct Writer {
    state: Arc<Mutex<Writes>>,
    body_first: bool,
}
impl AsyncWrite for Writer {
    fn poll_write(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let mut state = self.state.lock().unwrap();
        assert_eq!(
            state.head, !self.body_first,
            "a large body must have a reader before writing"
        );
        state.bytes += data.len();
        Poll::Ready(Ok(data.len()))
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        self.state.lock().unwrap().closed = true;
        Poll::Ready(Ok(()))
    }
}

#[tokio::test]
async fn whole_replies_publish_after_close_only_when_they_fit_one_window() {
    for length in [0, 42, TRANSPORT_BUFFER_BYTES, TRANSPORT_BUFFER_BYTES + 1] {
        let body_first = length <= TRANSPORT_BUFFER_BYTES;
        let state = Arc::new(Mutex::new(Writes::default()));
        let mut writer = Writer { state: state.clone(), body_first };
        send_body(&mut writer, bytes_body(vec![b'x'; length].into()), || {
            let mut state = state.lock().unwrap();
            assert_eq!(state.closed, body_first);
            assert_eq!(state.bytes, if body_first { length } else { 0 });
            state.head = true;
            Ok(())
        })
        .await
        .unwrap();
        let state = state.lock().unwrap();
        assert_eq!(state.bytes, length);
        assert!(state.closed && state.head);
    }
}
