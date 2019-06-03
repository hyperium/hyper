use std::error::Error as StdError;

use futures::{Async, Future, IntoFuture, Poll};

use body::Payload;
use super::{MakeService, Service};

/// An asynchronous constructor of `Service`s.
pub trait NewService {
    /// The `Payload` body of the `http::Request`.
    type ReqBody: Payload;

    /// The `Payload` body of the `http::Response`.
    type ResBody: Payload;

    /// The error type that can be returned by `Service`s.
    type Error: Into<Box<dyn StdError + Send + Sync>>;

    /// The resolved `Service` from `new_service()`.
    type Service: Service<
        ReqBody=Self::ReqBody,
        ResBody=Self::ResBody,
        Error=Self::Error,
    >;

    /// The future returned from `new_service` of a `Service`.
    type Future: Future<Item=Self::Service, Error=Self::InitError>;

    /// The error type that can be returned when creating a new `Service`.
    type InitError: Into<Box<dyn StdError + Send + Sync>>;

    #[doc(hidden)]
    fn poll_ready(&mut self) -> Poll<(), Self::InitError> {
        Ok(Async::Ready(()))
    }

    /// Create a new `Service`.
    fn new_service(&self) -> Self::Future;
}

impl<F, R, S> NewService for F
where
    F: Fn() -> R,
    R: IntoFuture<Item=S>,
    R::Error: Into<Box<dyn StdError + Send + Sync>>,
    S: Service,
{
    type ReqBody = S::ReqBody;
    type ResBody = S::ResBody;
    type Error = S::Error;
    type Service = S;
    type Future = R::Future;
    type InitError = R::Error;

    fn new_service(&self) -> Self::Future {
        (*self)().into_future()
    }
}

impl<N, Ctx> MakeService<Ctx> for N
where
    N: NewService,
{
    type ReqBody = N::ReqBody;
    type ResBody = N::ResBody;
    type Error = N::Error;
    type Service = N::Service;
    type Future = N::Future;
    type MakeError = N::InitError;

    fn poll_ready(&mut self) -> Poll<(), Self::MakeError> {
        NewService::poll_ready(self)
    }

    fn make_service(&mut self, _: Ctx) -> Self::Future {
        self.new_service()
    }
}

