use std::error::Error as StdError;
use std::fmt;

use futures::{Future, IntoFuture};

use body::Payload;
use super::Service;

/// An asynchronous constructor of `Service`s.
pub trait MakeService<Ctx> {
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
    type Future: Future<Item=Self::Service, Error=Self::MakeError>;

    /// The error type that can be returned when creating a new `Service`.
    type MakeError: Into<Box<StdError + Send + Sync>>;

    /// Create a new `Service`.
    fn make_service(&mut self, ctx: Ctx) -> Self::Future;
}


/// Create a `MakeService` from a function.
///
/// # Example
///
/// ```rust
/// use std::net::TcpStream;
/// use hyper::{Body, Request, Response};
/// use hyper::service::{make_service_fn, service_fn_ok};
///
/// let make_svc = make_service_fn(|socket: &TcpStream| {
///     let remote_addr = socket.peer_addr().unwrap();
///     service_fn_ok(move |_: Request<Body>| {
///         Response::new(Body::from(format!("Hello, {}", remote_addr)))
///     })
/// });
/// ```
pub fn make_service_fn<F, Ctx, Ret>(f: F) -> MakeServiceFn<F>
where
    F: Fn(&Ctx) -> Ret,
    Ret: IntoFuture,
{
    MakeServiceFn {
        f,
    }
}

// Not exported from crate as this will likely be replaced with `impl Service`.
pub struct MakeServiceFn<F> {
    f: F,
}

impl<'c, F, Ctx, Ret, ReqBody, ResBody> MakeService<&'c Ctx> for MakeServiceFn<F>
where
    F: Fn(&Ctx) -> Ret,
    Ret: IntoFuture,
    Ret::Item: Service<ReqBody=ReqBody, ResBody=ResBody>,
    Ret::Error: Into<Box<StdError + Send + Sync>>,
    ReqBody: Payload,
    ResBody: Payload,
{
    type ReqBody = ReqBody;
    type ResBody = ResBody;
    type Error = <Ret::Item as Service>::Error;
    type Service = Ret::Item;
    type Future = Ret::Future;
    type MakeError = Ret::Error;

    fn make_service(&mut self, ctx: &'c Ctx) -> Self::Future {
        (self.f)(ctx).into_future()
    }
}

impl<F> fmt::Debug for MakeServiceFn<F> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("MakeServiceFn")
            .finish()
    }
}

