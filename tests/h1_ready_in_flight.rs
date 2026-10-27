// Test: an HTTP/1 client connection must not report ready while a request is
// in flight on it.
//
// Signaling want whenever the dispatch queue reports `Pending` can restore
// readiness after a request has been sent. This happens with a concurrent
// send or when Tokio's coop budget is exhausted with a request still queued.
// Withdrawing that want when the request is taken is too late: a sender may
// already have observed readiness, only for its next send to be rejected.
//
// Drive the coop path deterministically and check readiness both before and
// after the queued request is taken.

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

    // Out of coop budget, the queue reports Pending with the request in it.
    // This must not signal readiness again, even before the request is taken.
    poll_fn(|cx| {
        while let Poll::Ready(restore) = tokio::task::coop::poll_proceed(cx) {
            restore.made_progress();
        }
        let _ = poll_conn(&mut conn, cx);
        assert!(
            sender.poll_ready(cx).is_pending(),
            "connection reports ready with a request still queued"
        );
        assert!(!sender.is_ready());
        Poll::Ready(())
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
