use std::future::Future;
use std::io::Cursor;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::{Buf, Bytes};
use futures_channel::{mpsc, oneshot};
use futures_core::{ready, Stream};
use h2::{Reason, RecvStream, SendStream};
use pin_project_lite::pin_project;

use super::ping::Recorder;
use super::SendBuf;
use crate::rt::{Read, ReadBufCursor, Write};

pub(super) fn pair<B>(
    send_stream: SendStream<SendBuf<B>>,
    recv_stream: RecvStream,
    ping: Recorder,
) -> (H2Upgraded, UpgradedSendStreamTask<B>) {
    let (tx, rx) = mpsc::channel(1);
    let (error_tx, error_rx) = oneshot::channel();
    let (reset_tx, reset_rx) = oneshot::channel();

    (
        H2Upgraded {
            send_stream: UpgradedSendStreamBridge {
                tx,
                error_rx,
                reset_tx: Some(reset_tx),
            },
            recv_stream,
            ping,
            buf: Bytes::new(),
        },
        UpgradedSendStreamTask {
            h2_tx: send_stream,
            rx,
            buffered: None,
            reset_rx: Some(reset_rx),
            error_tx: Some(error_tx),
        },
    )
}

pub(crate) struct H2Upgraded {
    ping: Recorder,
    send_stream: UpgradedSendStreamBridge,
    recv_stream: RecvStream,
    buf: Bytes,
}

struct UpgradedSendStreamBridge {
    tx: mpsc::Sender<Cursor<Box<[u8]>>>,
    error_rx: oneshot::Receiver<crate::Error>,
    reset_tx: Option<oneshot::Sender<Reason>>,
}

pin_project! {
    #[must_use = "futures do nothing unless polled"]
    pub struct UpgradedSendStreamTask<B> {
        #[pin]
        h2_tx: SendStream<SendBuf<B>>,
        #[pin]
        rx: mpsc::Receiver<Cursor<Box<[u8]>>>,
        buffered: Option<Cursor<Box<[u8]>>>,
        // Declared before `error_tx` so it is dropped first: once a writer
        // sees the task gone, a later reset request fails.
        reset_rx: Option<oneshot::Receiver<Reason>>,
        error_tx: Option<oneshot::Sender<crate::Error>>,
    }
}

// ===== impl UpgradedSendStreamTask =====

impl<B> UpgradedSendStreamTask<B>
where
    B: Buf,
{
    fn tick(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), crate::Error>> {
        let mut me = self.project();

        // this is a manual `select()` over 4 "futures", so we always need
        // to be sure they are ready and/or we are waiting notification of
        // one of the sides hanging up, so the task doesn't live around
        // longer than it's meant to.
        loop {
            match me.h2_tx.poll_reset(cx) {
                Poll::Ready(Ok(reason)) => {
                    trace!("stream received RST_STREAM: {:?}", reason);
                    return Poll::Ready(Err(crate::Error::new_body_write(::h2::Error::from(
                        reason,
                    ))));
                }
                Poll::Ready(Err(err)) => {
                    return Poll::Ready(Err(crate::Error::new_body_write(err)))
                }
                Poll::Pending => (),
            }

            // A requested reset wins over buffered or queued data, and over
            // the `END_STREAM` that a closed channel would send.
            let reset = match me.reset_rx.as_mut() {
                Some(reset_rx) => Pin::new(reset_rx).poll(cx),
                None => Poll::Pending,
            };
            match reset {
                Poll::Ready(Ok(reason)) => {
                    trace!("upgraded stream sending RST_STREAM: {:?}", reason);
                    me.h2_tx.send_reset(reason);
                    return Poll::Ready(Ok(()));
                }
                Poll::Ready(Err(_canceled)) => {
                    // The upgraded half was dropped without asking for one.
                    *me.reset_rx = None;
                }
                Poll::Pending => (),
            }

            // A write taken from the mpsc receiver waits here for h2 capacity,
            // and the next one isn't pulled until it has been handed to h2, so
            // the writer still sees h2 backpressure.
            if me.buffered.is_some() {
                // poll_capacity oddly needs a loop
                while me.h2_tx.capacity() == 0 {
                    match ready!(me.h2_tx.poll_capacity(cx)) {
                        Some(Ok(0)) => {}
                        Some(Ok(_)) => break,
                        Some(Err(e)) => return Poll::Ready(Err(crate::Error::new_body_write(e))),
                        None => {
                            // None means the stream is no longer in a
                            // streaming state, we either finished it
                            // somehow, or the remote reset us.
                            return Poll::Ready(Err(crate::Error::new_body_write(
                                "send stream capacity unexpectedly closed",
                            )));
                        }
                    }
                }

                let cursor = me.buffered.take().expect("checked is_some above");
                me.h2_tx
                    .send_data(SendBuf::Cursor(cursor), false)
                    .map_err(crate::Error::new_body_write)?;
                continue;
            }

            match me.rx.as_mut().poll_next(cx) {
                Poll::Ready(Some(cursor)) => {
                    // Only reserve capacity once there is something to send.
                    // Reserving while idle, even a single byte, pins that
                    // capacity on the connection-level window (#4003). As in
                    // `PipeToSendStream`, h2 raises the request to the
                    // buffered length inside `send_data`.
                    me.h2_tx.reserve_capacity(1);
                    *me.buffered = Some(cursor);
                }
                Poll::Ready(None) => {
                    me.h2_tx
                        .send_data(SendBuf::None, true)
                        .map_err(crate::Error::new_body_write)?;
                    return Poll::Ready(Ok(()));
                }
                Poll::Pending => {
                    return Poll::Pending;
                }
            }
        }
    }
}

impl<B> Future for UpgradedSendStreamTask<B>
where
    B: Buf,
{
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.as_mut().tick(cx) {
            Poll::Ready(Ok(())) => Poll::Ready(()),
            Poll::Ready(Err(err)) => {
                if let Some(tx) = self.error_tx.take() {
                    let _oh_well = tx.send(err);
                }
                Poll::Ready(())
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

// ===== impl H2Upgraded =====

impl H2Upgraded {
    /// Queues `RST_STREAM(reason)` in place of the `END_STREAM` that
    /// dropping or shutting down this stream sends.
    ///
    /// Returns `false` if a reset was already queued or the send task has
    /// already finished.
    pub(crate) fn reset(&mut self, reason: Reason) -> bool {
        match self.send_stream.reset_tx.take() {
            Some(reset_tx) => reset_tx.send(reason).is_ok(),
            None => false,
        }
    }
}

impl Read for H2Upgraded {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        mut read_buf: ReadBufCursor<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        if self.buf.is_empty() {
            self.buf = loop {
                match ready!(self.recv_stream.poll_data(cx)) {
                    None => return Poll::Ready(Ok(())),
                    Some(Ok(buf)) if buf.is_empty() && !self.recv_stream.is_end_stream() => {}
                    Some(Ok(buf)) => {
                        self.ping.record_data(buf.len());
                        break buf;
                    }
                    Some(Err(e)) => {
                        return Poll::Ready(match e.reason() {
                            Some(Reason::NO_ERROR) | Some(Reason::CANCEL) => Ok(()),
                            Some(Reason::STREAM_CLOSED) => {
                                Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, e))
                            }
                            _ => Err(h2_to_io_error(e)),
                        })
                    }
                }
            };
        }
        let cnt = std::cmp::min(self.buf.len(), read_buf.remaining());
        read_buf.put_slice(&self.buf[..cnt]);
        self.buf.advance(cnt);
        let _ = self.recv_stream.flow_control().release_capacity(cnt);
        Poll::Ready(Ok(()))
    }
}

impl Write for H2Upgraded {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        if buf.is_empty() {
            return Poll::Ready(Ok(0));
        }

        match self.send_stream.tx.poll_ready(cx) {
            Poll::Ready(Ok(())) => {}
            Poll::Ready(Err(_task_dropped)) => {
                // if the task dropped, check if there was an error
                // otherwise i guess its a broken pipe
                return match Pin::new(&mut self.send_stream.error_rx).poll(cx) {
                    Poll::Ready(Ok(reason)) => Poll::Ready(Err(io_error(reason))),
                    Poll::Ready(Err(_task_dropped)) => {
                        Poll::Ready(Err(std::io::ErrorKind::BrokenPipe.into()))
                    }
                    Poll::Pending => Poll::Pending,
                };
            }
            Poll::Pending => return Poll::Pending,
        }

        let n = buf.len();
        match self.send_stream.tx.start_send(Cursor::new(buf.into())) {
            Ok(()) => Poll::Ready(Ok(n)),
            Err(_task_dropped) => {
                // if the task dropped, check if there was an error
                // otherwise i guess its a broken pipe
                match Pin::new(&mut self.send_stream.error_rx).poll(cx) {
                    Poll::Ready(Ok(reason)) => Poll::Ready(Err(io_error(reason))),
                    Poll::Ready(Err(_task_dropped)) => {
                        Poll::Ready(Err(std::io::ErrorKind::BrokenPipe.into()))
                    }
                    Poll::Pending => Poll::Pending,
                }
            }
        }
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        match self.send_stream.tx.poll_ready(cx) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
            Poll::Ready(Err(_task_dropped)) => {
                // if the task dropped, check if there was an error
                // otherwise it was a clean close
                match Pin::new(&mut self.send_stream.error_rx).poll(cx) {
                    Poll::Ready(Ok(reason)) => Poll::Ready(Err(io_error(reason))),
                    Poll::Ready(Err(_task_dropped)) => Poll::Ready(Ok(())),
                    Poll::Pending => Poll::Pending,
                }
            }
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        self.send_stream.tx.close_channel();
        match Pin::new(&mut self.send_stream.error_rx).poll(cx) {
            Poll::Ready(Ok(reason)) => Poll::Ready(Err(io_error(reason))),
            Poll::Ready(Err(_task_dropped)) => Poll::Ready(Ok(())),
            Poll::Pending => Poll::Pending,
        }
    }
}

fn io_error(e: crate::Error) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, e)
}

fn h2_to_io_error(e: h2::Error) -> std::io::Error {
    if e.is_io() {
        e.into_io()
            .expect("h2 error reported io cause without an underlying io error")
    } else {
        std::io::Error::new(std::io::ErrorKind::Other, e)
    }
}

// Miri does not support the tokio runtime these tests need.
#[cfg(all(test, not(miri)))]
mod tests {
    use std::pin::Pin;

    use bytes::Bytes;
    use futures_util::future::poll_fn;
    use h2::Reason;

    use super::super::{ping, SendBuf};
    use crate::rt::Write;
    use crate::upgrade::Upgraded;

    /// Opens an HTTP/2 `CONNECT` stream over an in-memory pipe. Returns the
    /// server side as an `Upgraded`, with its send task running, and the
    /// client side's response body and request stream.
    async fn connect_stream() -> (Upgraded, h2::RecvStream, h2::SendStream<Bytes>) {
        let (client_io, server_io) = tokio::io::duplex(64 * 1024);

        let server = tokio::spawn(async move {
            let mut conn = h2::server::Builder::new()
                .handshake::<_, SendBuf<Bytes>>(server_io)
                .await
                .expect("server handshake");
            let (req, mut respond) = conn
                .accept()
                .await
                .expect("a CONNECT request")
                .expect("accept the CONNECT request");
            let send_stream = respond
                .send_response(http::Response::new(()), false)
                .expect("send the 200 response");
            tokio::spawn(async move { while conn.accept().await.is_some() {} });
            (req.into_body(), send_stream)
        });

        let (client, client_conn) = h2::client::handshake(client_io)
            .await
            .expect("client handshake");
        tokio::spawn(async move {
            let _ = client_conn.await;
        });
        let mut client = client.ready().await.expect("client ready");
        let req = http::Request::builder()
            .method(http::Method::CONNECT)
            .uri("example.com:443")
            .body(())
            .expect("CONNECT request");
        let (response, client_send) = client
            .send_request(req, false)
            .expect("send the CONNECT request");

        let (recv_stream, send_stream) = server.await.expect("server task");
        let response = response.await.expect("CONNECT response");
        assert_eq!(response.status(), http::StatusCode::OK);

        let (h2_upgraded, send_task) = super::pair(send_stream, recv_stream, ping::disabled());
        tokio::spawn(send_task);
        (
            Upgraded::new(h2_upgraded, Bytes::new()),
            response.into_body(),
            client_send,
        )
    }

    async fn write_all(upgraded: &mut Upgraded, mut buf: &[u8]) {
        while !buf.is_empty() {
            let n = poll_fn(|cx| Pin::new(&mut *upgraded).poll_write(cx, buf))
                .await
                .expect("write to the upgraded stream");
            buf = &buf[n..];
        }
        poll_fn(|cx| Pin::new(&mut *upgraded).poll_flush(cx))
            .await
            .expect("flush the upgraded stream");
    }

    /// Reads the next non-empty DATA chunk. h2 surfaces the empty DATA frame
    /// that carries `END_STREAM` as an empty chunk before `None`; an error
    /// (`RST_STREAM`) or real data is returned as is.
    async fn next_data(body: &mut h2::RecvStream) -> Option<Result<Bytes, h2::Error>> {
        loop {
            match body.data().await {
                Some(Ok(data)) if data.is_empty() => continue,
                other => return other,
            }
        }
    }

    #[tokio::test]
    async fn reset_sends_rst_stream_connect_error() {
        let (mut upgraded, mut client_body, _client_send) = connect_stream().await;

        assert!(upgraded.reset_with_connect_error());
        assert!(
            !upgraded.reset_with_connect_error(),
            "a reset is queued once"
        );
        // Dropping right after the reset must not turn it into END_STREAM.
        drop(upgraded);

        let err = client_body
            .data()
            .await
            .expect("the stream ends with an error, not END_STREAM")
            .expect_err("RST_STREAM, not DATA");
        assert_eq!(err.reason(), Some(Reason::CONNECT_ERROR));
    }

    #[tokio::test]
    async fn reset_after_data_sends_rst_stream_connect_error() {
        let (mut upgraded, mut client_body, _client_send) = connect_stream().await;

        write_all(&mut upgraded, b"tunnel bytes").await;
        let data = client_body
            .data()
            .await
            .expect("DATA")
            .expect("DATA, not an error");
        assert_eq!(&data[..], b"tunnel bytes");

        assert!(upgraded.reset_with_connect_error());

        let err = client_body
            .data()
            .await
            .expect("the stream ends with an error, not END_STREAM")
            .expect_err("RST_STREAM, not DATA");
        assert_eq!(err.reason(), Some(Reason::CONNECT_ERROR));
        drop(upgraded);
    }

    #[tokio::test]
    async fn drop_without_reset_ends_the_stream_cleanly() {
        let (upgraded, mut client_body, _client_send) = connect_stream().await;

        drop(upgraded);

        assert!(
            next_data(&mut client_body).await.is_none(),
            "a plain drop still sends END_STREAM"
        );
        assert!(client_body.is_end_stream());
    }

    #[tokio::test]
    async fn reset_after_completed_shutdown_does_nothing() {
        let (mut upgraded, mut client_body, _client_send) = connect_stream().await;

        poll_fn(|cx| Pin::new(&mut upgraded).poll_shutdown(cx))
            .await
            .expect("shutdown");

        assert!(!upgraded.reset_with_connect_error());
        assert!(
            next_data(&mut client_body).await.is_none(),
            "the completed shutdown already sent END_STREAM"
        );
    }
}
