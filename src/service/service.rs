use std::error::Error as StdError;
use std::fmt;
use std::marker::PhantomData;

use crate::body::Payload;
use crate::common::{Future, Never, Poll, task};
use crate::{Request, Response};

/// An asynchronous function from `Request` to `Response`.
pub trait Service<ReqBody>: sealed::Sealed<ReqBody> {
    /// The `Payload` body of the `http::Response`.
    type ResBody: Payload;

    /// The error type that can occur within this `Service`.
    ///
    /// Note: Returning an `Error` to a hyper server will cause the connection
    /// to be abruptly aborted. In most cases, it is better to return a `Response`
    /// with a 4xx or 5xx status code.
    type Error: Into<Box<dyn StdError + Send + Sync>>;

    /// The `Future` returned by this `Service`.
    type Future: Future<Output=Result<Response<Self::ResBody>, Self::Error>>;

    /// Returns `Ready` when the service is able to process requests.
    ///
    /// The implementation of this method is allowed to return a `Ready` even if
    /// the service is not ready to process. In this case, the future returned
    /// from `call` will resolve to an error.
    fn poll_ready(&mut self, cx: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    /// Calls this `Service` with a request, returning a `Future` of the response.
    fn call(&mut self, req: Request<ReqBody>) -> Self::Future;
}

impl<T, B1, B2> Service<B1> for T 
where 
    T: tower_service::Service<Request<B1>, Response = Response<B2>>,
    B2: Payload,
    T::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    type ResBody = B2;

    type Error = T::Error;
    type Future = T::Future;

    fn poll_ready(&mut self, cx: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        tower_service::Service::poll_ready(self, cx)
    }

    fn call(&mut self, req: Request<B1>) -> Self::Future {
        tower_service::Service::call(self, req)
    }
}

impl<T, B1, B2> sealed::Sealed<B1> for T 
where 
    T: tower_service::Service<Request<B1>, Response = Response<B2>>,
    B2: Payload,
{}

mod sealed {
    pub trait Sealed<T> {}
}


/// Create a `Service` from a function.
///
/// # Example
///
/// ```rust
/// use hyper::{Body, Request, Response, Version};
/// use hyper::service::service_fn;
///
/// let service = service_fn(|req: Request<Body>| async move{
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
    S: Future,
{
    ServiceFn {
        f,
        _req: PhantomData,
    }
}

// Not exported from crate as this will likely be replaced with `impl Service`.
pub struct ServiceFn<F, R> {
    f: F,
    _req: PhantomData<fn(R)>,
}

impl<F, ReqBody, Ret, ResBody, E> tower_service::Service<crate::Request<ReqBody>> for ServiceFn<F, ReqBody>
where
    F: FnMut(Request<ReqBody>) -> Ret,
    ReqBody: Payload,
    Ret: Future<Output=Result<Response<ResBody>, E>>,
    E: Into<Box<dyn StdError + Send + Sync>>,
    ResBody: Payload,
{
    type Response = crate::Response<ResBody>;
    type Error = E;
    type Future = Ret;

    fn poll_ready(&mut self, cx: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        (self.f)(req)
    }
}

impl<F, R> fmt::Debug for ServiceFn<F, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("impl Service")
            .finish()
    }
}
