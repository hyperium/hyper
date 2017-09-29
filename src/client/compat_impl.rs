use futures::{Future, Poll, Stream};
use http_types;
use tokio_service::Service;

use client::{Connect, Client, FutureResponse};
use error::Error;
use proto::Body;

/// A Client to make outgoing HTTP requests.
#[derive(Debug)]
pub struct CompatClient<C, B = Body> {
    inner: Client<C, B>
}

pub fn client<C, B>(client: Client<C, B>) -> CompatClient<C, B> {
    CompatClient { inner: client }
}

impl<C, B> Service for CompatClient<C, B>
where C: Connect,
      B: Stream<Error=Error> + 'static,
      B::Item: AsRef<[u8]>,
{
    type Request = http_types::Request<B>;
    type Response = http_types::Response<Body>;
    type Error = Error;
    type Future = CompatFutureResponse;

    fn call(&self, req: Self::Request) -> Self::Future {
        future(self.inner.call(req.into()))
    }
}

/// A `Future` that will resolve to an `http::Response`.
#[must_use = "futures do nothing unless polled"]
#[derive(Debug)]
pub struct CompatFutureResponse {
    inner: FutureResponse
}

pub fn future(fut: FutureResponse) -> CompatFutureResponse {
    CompatFutureResponse { inner: fut }
}

impl Future for CompatFutureResponse {
    type Item = http_types::Response<Body>;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Error> {
        self.inner.poll()
            .map(|a| a.map(|r| r.into()))
    }
}
