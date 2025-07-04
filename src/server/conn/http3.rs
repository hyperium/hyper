//! HTTP/3 Server Connections

use std::error::Error as StdError;

use pin_project_lite::pin_project;

use crate::body::{Body, Incoming as IncomingBody};
use crate::rt::quic;
use crate::service::HttpService;

pin_project! {
    /// A Future representing an HTTP/3 connection.
    #[must_use = "futures do nothing unless polled"]
    pub struct Connection<Q, S, E> {
        _i: (Q, S, E),
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
        Self {
            exec,
        }
    }

    /// Bind a connection together with a [`Service`](crate::service::Service).
    ///
    /// This returns a Future that must be polled in order for HTTP to be
    /// driven on the connection.
    pub fn serve_connection<S, Q, Bd>(&self, quic: Q, service: S) -> Connection<Q, S, E>
    where
        S: HttpService<IncomingBody, ResBody = Bd>,
        S::Error: Into<Box<dyn StdError + Send + Sync>>,
        Bd: Body + 'static,
        Bd::Error: Into<Box<dyn StdError + Send + Sync>>,
        Q: quic::Connection<Bd>,
        //E: Http2ServerConnExec<S::Future, Bd>,
        E: Clone,
    {
        Connection {
            _i: (quic, service, self.exec.clone()),
        }
    }
}
