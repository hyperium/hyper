use std::error::Error as StdError;

use futures::{Future, IntoFuture};

use tokio::io::{AsyncRead, AsyncWrite};
use futures::Stream;
use body::Payload;
use super::Service;
use server::conn::AddrIncoming;

/// An asynchronous constructor of `Service`s.
pub trait NewService {
    /// Incoming connections.
    type Incoming: AsyncRead + AsyncWrite;

    /// The `Payload` body of the `http::Request`.
    type ReqBody: Payload;

    /// The `Payload` body of the `http::Response`.
    type ResBody: Payload;

    /// The error type that can be returned by `Service`s.
    type Error: Into<Box<StdError + Send + Sync>>;

    /// The resolved `Service` from `new_service()`.
    type Service: Service<
        ReqBody=Self::ReqBody,
        ResBody=Self::ResBody,
        Error=Self::Error,
    >;

    /// The future returned from `new_service` of a `Service`.
    type Future: Future<Item=Self::Service, Error=Self::InitError>;

    /// The error type that can be returned when creating a new `Service`.
    type InitError: Into<Box<StdError + Send + Sync>>;

    /// Create a new `Service`.
    fn new_service(&self, remote: &Self::Incoming) -> Self::Future;
}

impl<F, R, S> NewService for F
where
    F: Fn(&<AddrIncoming as Stream>::Item) -> R,
    R: IntoFuture<Item=S>,
    R::Error: Into<Box<StdError + Send + Sync>>,
    S: Service,
{
    type Incoming = <AddrIncoming as Stream>::Item;
    type ReqBody = S::ReqBody;
    type ResBody = S::ResBody;
    type Error = S::Error;
    type Service = S;
    type Future = R::Future;
    type InitError = R::Error;


    fn new_service(&self, remote: &Self::Incoming) -> Self::Future {
        (*self)(remote).into_future()
    }
}

