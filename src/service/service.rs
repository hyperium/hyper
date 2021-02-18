use std::error::Error as StdError;
use std::fmt;
use std::marker::PhantomData;

use futures::{future, Async, Future, IntoFuture, Poll};

use body::Payload;
use common::Never;
use ::{Request, Response};

/// An asynchronous function from `Request` to `Response`.
pub trait Service {
    /// The `Payload` body of the `http::Request`.
    type ReqBody: Payload;

    /// The `Payload` body of the `http::Response`.
    type ResBody: Payload;

    /// The error type that can occur within this `Service`.
    ///
    /// Note: Returning an `Error` to a hyper server will cause the connection
    /// to be abruptly aborted. In most cases, it is better to return a `Response`
    /// with a 4xx or 5xx status code.
    type Error: Into<Box<dyn StdError + Send + Sync>>;

    /// The `Future` returned by this `Service`.
    type Future: Future<Item=Response<Self::ResBody>, Error=Self::Error>;

    /// Returns `Ready` when the service is able to process requests.
    ///
    /// The implementation of this method is allowed to return a `Ready` even if
    /// the service is not ready to process. In this case, the future returned
    /// from `call` will resolve to an error.
    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
        Ok(Async::Ready(()))
    }

    /// Calls this `Service` with a request, returning a `Future` of the response.
    fn call(&mut self, req: Request<Self::ReqBody>) -> Self::Future;
}


/// Create a `Service` from a function.
///
/// # Example
///
/// ```rust
/// use hyper::{Body, Request, Response, Version};
/// use hyper::service::service_fn;
///
/// let service = service_fn(|req: Request<Body>| {
///     if req.version() == Version::HTTP_11 {
///         Ok(Response::new(Body::from("Hello World")))
///     } else {
///         // Note: it's usually better to return a Response
///         // with an appropriate StatusCode instead of an Err.
///         Err("not HTTP/1.1, abort connection")
///     }
/// });
/// ```
pub fn service_fn<F, R, S>(f: F) -> ServiceFn<F, R>
where
    F: FnMut(Request<R>) -> S,
    S: IntoFuture,
{
    ServiceFn {
        f,
        _req: PhantomData,
    }
}

/// Create a `Service` from a function that never errors.
///
/// # Example
///
/// ```rust
/// use hyper::{Body, Request, Response};
/// use hyper::service::service_fn_ok;
///
/// let service = service_fn_ok(|req: Request<Body>| {
///     println!("request: {} {}", req.method(), req.uri());
///     Response::new(Body::from("Hello World"))
/// });
/// ```
pub fn service_fn_ok<F, R, S>(f: F) -> ServiceFnOk<F, R>
where
    F: FnMut(Request<R>) -> Response<S>,
    S: Payload,
{
    ServiceFnOk {
        f,
        _req: PhantomData,
    }
}

// Not exported from crate as this will likely be replaced with `impl Service`.
pub struct ServiceFn<F, R> {
    f: F,
    _req: PhantomData<fn(R)>,
}

impl<F, ReqBody, Ret, ResBody> Service for ServiceFn<F, ReqBody>
where
    F: FnMut(Request<ReqBody>) -> Ret,
    ReqBody: Payload,
    Ret: IntoFuture<Item=Response<ResBody>>,
    Ret::Error: Into<Box<dyn StdError + Send + Sync>>,
    ResBody: Payload,
{
    type ReqBody = ReqBody;
    type ResBody = ResBody;
    type Error = Ret::Error;
    type Future = Ret::Future;

    fn call(&mut self, req: Request<Self::ReqBody>) -> Self::Future {
        (self.f)(req).into_future()
    }
}

impl<F, R> IntoFuture for ServiceFn<F, R> {
    type Future = future::FutureResult<Self::Item, Self::Error>;
    type Item = Self;
    type Error = Never;

    fn into_future(self) -> Self::Future {
        future::ok(self)
    }
}

impl<F, R> fmt::Debug for ServiceFn<F, R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("impl Service")
            .finish()
    }
}

// Not exported from crate as this will likely be replaced with `impl Service`.
pub struct ServiceFnOk<F, R> {
    f: F,
    _req: PhantomData<fn(R)>,
}

impl<F, ReqBody, ResBody> Service for ServiceFnOk<F, ReqBody>
where
    F: FnMut(Request<ReqBody>) -> Response<ResBody>,
    ReqBody: Payload,
    ResBody: Payload,
{
    type ReqBody = ReqBody;
    type ResBody = ResBody;
    type Error = Never;
    type Future = future::FutureResult<Response<ResBody>, Never>;

    fn call(&mut self, req: Request<Self::ReqBody>) -> Self::Future {
        future::ok((self.f)(req))
    }
}

impl<F, R> IntoFuture for ServiceFnOk<F, R> {
    type Future = future::FutureResult<Self::Item, Self::Error>;
    type Item = Self;
    type Error = Never;

    fn into_future(self) -> Self::Future {
        future::ok(self)
    }
}

impl<F, R> fmt::Debug for ServiceFnOk<F, R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("impl Service")
            .finish()
    }
}

fn _assert_fn_mut() {
    fn assert_service<T: Service>(_t: &T) {}

    let mut val = 0;

    let svc = service_fn(move |_req: Request<::Body>| {
        val += 1;
        future::ok::<_, Never>(Response::new(::Body::from(val.to_string())))
    });

    assert_service(&svc);

    let svc = service_fn_ok(move |_req: Request<::Body>| {
        val += 1;
        Response::new(::Body::from(val.to_string()))
    });

    assert_service(&svc);
}
