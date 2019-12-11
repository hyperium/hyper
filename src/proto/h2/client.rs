use futures_channel::{mpsc, oneshot};
use futures_util::future::{self, Either, FutureExt as _, TryFutureExt as _};
use futures_util::stream::StreamExt as _;
use h2::client::{Builder, SendRequest};
use tokio::io::{AsyncRead, AsyncWrite};

use super::{PipeToSendStream, SendBuf};
use crate::body::Payload;
use crate::common::{task, Exec, Future, Never, Pin, Poll};
use crate::headers;
use crate::headers::content_length_parse_all;
use crate::proto::Dispatched;
use crate::{Body, Request, Response};

type ClientRx<B> = crate::client::dispatch::Receiver<Request<B>, Response<Body>>;

///// An mpsc channel is used to help notify the `Connection` task when *all*
///// other handles to it have been dropped, so that it can shutdown.
type ConnDropRef = mpsc::Sender<Never>;

///// A oneshot channel watches the `Connection` task, and when it completes,
///// the "dispatch" task will be notified and can shutdown sooner.
type ConnEof = oneshot::Receiver<Never>;

pub(crate) async fn handshake<T, B>(
    io: T,
    req_rx: ClientRx<B>,
    builder: &Builder,
    exec: Exec,
) -> crate::Result<ClientTask<B>>
where
    T: AsyncRead + AsyncWrite + Send + Unpin + 'static,
    B: Payload,
{
    let (h2_tx, conn) = builder
        .handshake::<_, SendBuf<B::Data>>(io)
        .await
        .map_err(crate::Error::new_h2)?;

    // An mpsc channel is used entirely to detect when the
    // 'Client' has been dropped. This is to get around a bug
    // in h2 where dropping all SendRequests won't notify a
    // parked Connection.
    let (conn_drop_ref, rx) = mpsc::channel(1);
    let (cancel_tx, conn_eof) = oneshot::channel();

    let conn_drop_rx = rx.into_future().map(|(item, _rx)| match item {
        Some(never) => match never {},
        None => (),
    });

    let conn = conn.map_err(|e| debug!("connection error: {}", e));

    let conn_task = async move {
        match future::select(conn, conn_drop_rx).await {
            Either::Left(_) => {
                // ok or err, the `conn` has finished
            }
            Either::Right(((), conn)) => {
                // mpsc has been dropped, hopefully polling
                // the connection some more should start shutdown
                // and then close
                trace!("send_request dropped, starting conn shutdown");
                drop(cancel_tx);
                let _ = conn.await;
            }
        }
    };

    exec.execute(conn_task);

    Ok(ClientTask {
        conn_drop_ref,
        conn_eof,
        executor: exec,
        h2_tx,
        req_rx,
    })
}

pub(crate) struct ClientTask<B>
where
    B: Payload,
{
    conn_drop_ref: ConnDropRef,
    conn_eof: ConnEof,
    executor: Exec,
    h2_tx: SendRequest<SendBuf<B::Data>>,
    req_rx: ClientRx<B>,
}

impl<B> Future for ClientTask<B>
where
    B: Payload + 'static,
{
    type Output = crate::Result<Dispatched>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        loop {
            match ready!(self.h2_tx.poll_ready(cx)) {
                Ok(()) => (),
                Err(err) => {
                    return if err.reason() == Some(::h2::Reason::NO_ERROR) {
                        trace!("connection gracefully shutdown");
                        Poll::Ready(Ok(Dispatched::Shutdown))
                    } else {
                        Poll::Ready(Err(crate::Error::new_h2(err)))
                    };
                }
            };

            match Pin::new(&mut self.req_rx).poll_next(cx) {
                Poll::Ready(Some((req, cb))) => {
                    // check that future hasn't been canceled already
                    if cb.is_canceled() {
                        trace!("request callback is canceled");
                        continue;
                    }
                    let (head, body) = req.into_parts();
                    let mut req = ::http::Request::from_parts(head, ());
                    super::strip_connection_headers(req.headers_mut(), true);
                    if let Some(len) = body.size_hint().exact() {
                        headers::set_content_length_if_missing(req.headers_mut(), len);
                    }
                    let eos = body.is_end_stream();
                    let (fut, body_tx) = match self.h2_tx.send_request(req, eos) {
                        Ok(ok) => ok,
                        Err(err) => {
                            debug!("client send request error: {}", err);
                            cb.send(Err((crate::Error::new_h2(err), None)));
                            continue;
                        }
                    };

                    if !eos {
                        let mut pipe = Box::pin(PipeToSendStream::new(body, body_tx)).map(|res| {
                            if let Err(e) = res {
                                debug!("client request body error: {}", e);
                            }
                        });

                        // eagerly see if the body pipe is ready and
                        // can thus skip allocating in the executor
                        match Pin::new(&mut pipe).poll(cx) {
                            Poll::Ready(_) => (),
                            Poll::Pending => {
                                let conn_drop_ref = self.conn_drop_ref.clone();
                                let pipe = pipe.map(move |x| {
                                    drop(conn_drop_ref);
                                    x
                                });
                                self.executor.execute(pipe);
                            }
                        }
                    }

                    let fut = fut.map(move |result| match result {
                        Ok(res) => {
                            let content_length = content_length_parse_all(res.headers());
                            let res = res.map(|stream| crate::Body::h2(stream, content_length));
                            Ok(res)
                        }
                        Err(err) => {
                            debug!("client response error: {}", err);
                            Err((crate::Error::new_h2(err), None))
                        }
                    });
                    self.executor.execute(cb.send_when(fut));
                    continue;
                }

                Poll::Ready(None) => {
                    trace!("client::dispatch::Sender dropped");
                    return Poll::Ready(Ok(Dispatched::Shutdown));
                }

                Poll::Pending => match ready!(Pin::new(&mut self.conn_eof).poll(cx)) {
                    Ok(never) => match never {},
                    Err(_conn_is_eof) => {
                        trace!("connection task is closed, closing dispatch task");
                        return Poll::Ready(Ok(Dispatched::Shutdown));
                    }
                },
            }
        }
    }
}
