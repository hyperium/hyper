// Test: an HTTP/1 client connection must not report ready while a request is
// in flight on it.
//
// The dispatch `Receiver` signals want when its queue reports `Pending`, and
// `SendRequest::is_ready` reports that want. The signal can go stale: the
// connection task can find its queue empty and a request land before it
// signals, or tokio's coop budget can make the queue report `Pending` with a
// request already in it. The request is then taken with want still set, so
// `is_ready` stays true until the response is complete. Pools that return a
// connection on `is_ready`, such as hyper-util's legacy client, then give that
// connection to the next request, which waits behind the whole response.
//
// The coop path is deterministic, so the test drives that one.

use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use futures_util::future::poll_fn;
use http_body_util::Empty;
use hyper::client::conn::http1::{self, Connection};
use hyper::rt::{Read, ReadBufCursor, Write};
use hyper::Request;

/// Accepts every write and never answers, so a sent request stays in flight.
struct SilentIo;

impl Read for SilentIo {
    fn poll_read(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        _: ReadBufCursor<'_>,
    ) -> Poll<io::Result<()>> {
        Poll::Pending
    }
}

impl Write for SilentIo {
    fn poll_write(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

/// Runs the connection task once; it never finishes, as the peer never answers.
fn poll_conn(conn: &mut Connection<SilentIo, Empty<Bytes>>, cx: &mut Context<'_>) -> Poll<()> {
    assert!(Pin::new(conn).poll(cx).is_pending());
    Poll::Ready(())
}

#[tokio::test]
async fn h1_connection_is_not_ready_while_a_request_is_in_flight() {
    let (mut sender, mut conn) = http1::handshake(SilentIo).await.unwrap();

    // Idle, the connection finds its queue empty and signals it is ready.
    poll_fn(|cx| poll_conn(&mut conn, cx)).await;
    assert!(sender.is_ready());

    // Kept alive: dropping the response future cancels the request.
    let _response = sender.send_request(Request::new(Empty::new()));
    assert!(!sender.is_ready());

    // Out of coop budget, the queue reports Pending with the request in it, so
    // the connection signals ready again. A connection task preempted between
    // finding its queue empty and signaling leaves the same stale signal.
    poll_fn(|cx| {
        while let Poll::Ready(restore) = tokio::task::coop::poll_proceed(cx) {
            restore.made_progress();
        }
        poll_conn(&mut conn, cx)
    })
    .await;
    tokio::task::yield_now().await;

    // With a fresh budget it takes and writes the request, which then waits
    // for a response that never comes.
    poll_fn(|cx| poll_conn(&mut conn, cx)).await;
    assert!(
        !sender.is_ready(),
        "connection reports ready with a request in flight"
    );
}
