use std::io::{Error as IoError};

use futures::{Future, Poll};
use http_types;
use tokio_service::{NewService, Service};

use error::Error;
use proto::Body;
use proto::request::Request;
use proto::response::Response;

/// Wraps a `Future` returning an `http::Response` into
/// a `Future` returning a `hyper::server::Response`.
#[derive(Debug)]
pub struct CompatFuture<F> {
    future: F
}

impl<F, Bd> Future for CompatFuture<F>
    where F: Future<Item=http_types::Response<Bd>, Error=Error>
{
    type Item = Response<Bd>;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.future.poll()
            .map(|a| a.map(|res| res.into()))
    }
}

/// Wraps a `Service` taking an `http::Request` and returning
/// an `http::Response` into a `Service` taking a `hyper::server::Request`,
/// and returning a `hyper::server::Response`.
#[derive(Debug)]
pub struct CompatService<S> {
    service: S
}

pub fn service<S>(service: S) -> CompatService<S> {
    CompatService { service: service }
}

impl<S, Bd> Service for CompatService<S>
    where S: Service<Request=http_types::Request<Body>, Response=http_types::Response<Bd>, Error=Error>
{
    type Request = Request;
    type Response = Response<Bd>;
    type Error = Error;
    type Future = CompatFuture<S::Future>;

    fn call(&self, req: Self::Request) -> Self::Future {
        CompatFuture {
            future: self.service.call(req.into())
        }
    }
}

/// Wraps a `NewService` taking an `http::Request` and returning
/// an `http::Response` into a `NewService` taking a `hyper::server::Request`,
/// and returning a `hyper::server::Response`.
#[derive(Debug)]
pub struct NewCompatService<S> {
    new_service: S
}

pub fn new_service<S>(new_service: S) -> NewCompatService<S> {
    NewCompatService { new_service: new_service }
}

impl<S, Bd> NewService for NewCompatService<S>
    where S: NewService<Request=http_types::Request<Body>, Response=http_types::Response<Bd>, Error=Error>
{
    type Request = Request;
    type Response = Response<Bd>;
    type Error = Error;
    type Instance = CompatService<S::Instance>;

    fn new_service(&self) -> Result<Self::Instance, IoError> {
        self.new_service.new_service()
            .map(service)
    }
}
