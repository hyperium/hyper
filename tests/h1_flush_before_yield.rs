// Test: `poll_loop` must never yield leaving buffered bytes unflushed.
//
// `poll_loop`'s main path always calls `poll_flush` after `poll_write`. The
// "wants_write_again" re-check added a *second* `poll_write` whose `Pending`
// returned straight out of the loop, skipping that flush.
//
// That second write can buffer bytes before it pends. A response body that
// reaches end-of-stream between the two write polls has its end-of-message
// buffered by `end_body()`, and the write then pends on the *next* message
// (`poll_msg`). Returning there strands the terminating chunk in the write
// buffer: the wake-ups the connection then waits on are for reads, so nothing
// flushes it and the peer waits for a response that is already written but
// never sent.
//
// The body below drives exactly that interleaving deterministically: one data
// frame, then `Pending`, then end-of-stream on the very next poll, both of
// which happen inside a single `poll_loop` iteration.

use std::convert::Infallible;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use bytes::Bytes;
use hyper::body::{Body, Frame};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::Response;
use support::TokioIo;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

mod support;

/// Yields one data frame, then pends once, then ends the stream.
///
/// The `Pending` deliberately arranges no wake-up: it stands in for a body
/// whose readiness changes between hyper's two write polls of the same
/// `poll_loop` iteration, which is what leaves the end of the message buffered
/// by the re-check write.
#[derive(Default)]
struct PendOnceThenEnd {
    polls: u8,
}

impl Body for PendOnceThenEnd {
    type Data = Bytes;
    type Error = Infallible;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        self.polls += 1;
        match self.polls {
            1 => Poll::Ready(Some(Ok(Frame::data(Bytes::from_static(b"hello"))))),
            2 => Poll::Pending,
            _ => Poll::Ready(None),
        }
    }
}

#[tokio::test]
async fn h1_server_flushes_end_of_body_buffered_by_write_recheck() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let service = service_fn(|_req| async {
            Ok::<_, Infallible>(Response::new(PendOnceThenEnd::default()))
        });
        let _ = http1::Builder::new()
            .serve_connection(TokioIo::new(socket), service)
            .await;
    });

    let mut client = TcpStream::connect(addr).await.unwrap();
    client
        .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .unwrap();

    // The whole response must arrive, terminating chunk included. Before the
    // fix the body's chunk arrives but `0\r\n\r\n` never does, so this read
    // waits forever.
    let mut received = Vec::new();
    let read = timeout(Duration::from_secs(5), async {
        let mut buf = [0u8; 256];
        loop {
            let n = client.read(&mut buf).await.unwrap();
            if n == 0 {
                break;
            }
            received.extend_from_slice(&buf[..n]);
            if received.ends_with(b"\r\n0\r\n\r\n") {
                break;
            }
        }
    })
    .await;

    assert!(
        read.is_ok(),
        "response never completed; got {:?}",
        String::from_utf8_lossy(&received)
    );
    let received = String::from_utf8_lossy(&received);
    assert!(
        received.ends_with("\r\n0\r\n\r\n"),
        "missing terminating chunk; got {received:?}"
    );
    assert!(
        received.contains("hello"),
        "missing body content; got {received:?}"
    );
}
