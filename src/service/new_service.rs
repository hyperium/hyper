use std::error::Error as StdError;

use futures::{Future, IntoFuture};

use body::Payload;
use super::Service;

/// An asynchronous constructor of `Service`s.
pub trait NewService {
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

    /// The error type that can be returned when creating a new `Service.
    type InitError: Into<Box<StdError + Send + Sync>>;

    /// Create a new `Service`.
    fn new_service(&self) -> Self::Future;
}

impl<F, R, S> NewService for F
where
    F: Fn() -> R,
    R: IntoFuture<Item=S>,
    R::Error: Into<Box<StdError + Send + Sync>>,
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

