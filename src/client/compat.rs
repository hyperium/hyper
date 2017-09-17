use futures::{Future, Poll};
use http_types;

use client::FutureResponse;
use error::Error;
use http::Body;

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