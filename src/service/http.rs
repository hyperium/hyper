use std::error::Error as StdError;
use std::future::Future;

use crate::body::Body;
use crate::service::service::Service;
use crate::{Request, Response};

/// An asynchronous function from `Request` to `Response`.
pub trait HttpService<ReqBody>: sealed::Sealed<ReqBody> {
    /// The `Body` body of the `http::Response`.
    type ResBody: Body;

    /// The error type that can occur within this `Service`.
    ///
    /// Note: Returning an `Error` to a hyper server will cause the connection
    /// to be abruptly aborted. In most cases, it is better to return a `Response`
    /// with a 4xx or 5xx status code.
    type Error: Into<Box<dyn StdError + Send + Sync>>;

    /// The `Future` returned by this `Service`.
    type Future: Future<Output = Result<Response<Self::ResBody>, Self::Error>>;

    #[doc(hidden)]
    fn call(&mut self, req: Request<ReqBody>) -> Self::Future;
}

impl<T, B1, B2> HttpService<B1> for T
where
    T: Service<Request<B1>, Response = Response<B2>>,
    B2: Body,
    T::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    type ResBody = B2;

    type Error = T::Error;
    type Future = T::Future;

    fn call(&mut self, req: Request<B1>) -> Self::Future {
        Service::call(self, req)
    }
}

impl<T, B1, B2> sealed::Sealed<B1> for T
where
    T: Service<Request<B1>, Response = Response<B2>>,
    B2: Body,
{
}

mod sealed {
    pub trait Sealed<T> {}
}
