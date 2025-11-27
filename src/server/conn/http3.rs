//! HTTP/3 Server Connections

use std::error::Error as StdError;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_core::ready;
use pin_project_lite::pin_project;

use crate::body::{Body, Incoming as IncomingBody};
use crate::proto;
use crate::rt::quic;
use crate::service::HttpService;

pin_project! {
    /// A Future representing an HTTP/3 connection.
    #[must_use = "futures do nothing unless polled"]
    pub struct Connection<Q, S, B, E>
        where
            Q: crate::rt::quic::Connection<B>,
            Q: Clone,
            Q: Unpin,
            B: bytes::Buf,
    {
        // _i: (Q, S, E),
        conn: proto::h3::Server<Q, S, B, E>
    }
}

impl<Q, Bd, B, S, E> Future for Connection<Q, S, B, E>
where
    S: HttpService<IncomingBody, ResBody = Bd>,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
    Q: crate::rt::quic::Connection<B> + Unpin + Clone,
    B: bytes::Buf,
    Bd: Body + 'static,
    Bd::Error: Into<Box<dyn StdError + Send + Sync>>,
    // E: Http2ServerConnExec<S::Future, B>,
{
    type Output = crate::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match ready!(Pin::new(&mut self.conn).poll(cx)) {
            Ok(_done) => {
                //TODO: the proto::h2::Server no longer needs to return
                //the Dispatched enum
                Poll::Ready(Ok(()))
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}

/// A configuration builder for HTTP/3 server connections.
///
/// **Note**: The default values of options are *not considered stable*. They
/// are subject to change at any time.
#[derive(Clone, Debug)]
pub struct Builder<E> {
    exec: E,
}

// ===== impl Connection =====

// ===== impl Builder =====

impl<E> Builder<E> {
    /// Create a new connection builder.
    pub fn new(exec: E) -> Self {
        Self { exec }
    }

    /// Bind a connection together with a [`Service`](crate::service::Service).
    ///
    /// This returns a Future that must be polled in order for HTTP to be
    /// driven on the connection.
    pub fn serve_connection<S, Q, Bd, B>(&self, quic: Q, service: S) -> Connection<Q, S, B, E>
    where
        S: HttpService<IncomingBody, ResBody = Bd>,
        S::Error: Into<Box<dyn StdError + Send + Sync>>,
        B: bytes::Buf,
        Bd: Body + 'static,
        Bd::Error: Into<Box<dyn StdError + Send + Sync>>,
        Q: quic::Connection<B> + Unpin + Clone,
        //E: Http2ServerConnExec<S::Future, Bd>,
        E: Clone,
    {
        Connection {
            conn: todo!(), // _i: (quic, service, self.exec.clone()),
        }
    }
}
