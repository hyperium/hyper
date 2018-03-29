use std::marker::PhantomData;
use std::sync::Arc;

use futures::{Future, IntoFuture};

/// An asynchronous function from `Request` to a `Response`.
pub trait Service {
    /// Requests handled by the service.
    type Request;
    /// Responses given by the service.
    type Response;
    /// Errors produced by the service.
    type Error;
    /// The future response value.
    type Future: Future<Item = Self::Response, Error = Self::Error>;
    /// Process the request and return the response asynchronously.
    fn call(&self, req: Self::Request) -> Self::Future;
}

impl<S: Service + ?Sized> Service for Arc<S> {
    type Request = S::Request;
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn call(&self, request: S::Request) -> S::Future {
        (**self).call(request)
    }
}

/// Creates new `Service` values.
pub trait NewService {
    /// Requests handled by the service.
    type Request;
    /// Responses given by the service.
    type Response;
    /// Errors produced by the service.
    type Error;
    /// The `Service` value created by this factory.
    type Instance: Service<Request = Self::Request, Response = Self::Response, Error = Self::Error>;
    /// Create and return a new service value.
    fn new_service(&self) -> ::std::io::Result<Self::Instance>;
}

impl<F, R> NewService for F
    where F: Fn() -> ::std::io::Result<R>,
          R: Service,
{
    type Request = R::Request;
    type Response = R::Response;
    type Error = R::Error;
    type Instance = R;

    fn new_service(&self) -> ::std::io::Result<R> {
        (*self)()
    }
}

/// Create a `Service` from a function.
pub fn service_fn<F, R, S>(f: F) -> ServiceFn<F, R>
where
    F: Fn(R) -> S,
    S: IntoFuture,
{
    ServiceFn {
        f: f,
        _req: PhantomData,
    }
}

/// Create a `NewService` by sharing references of `service.
pub fn const_service<S>(service: S) -> ConstService<S> {
    ConstService {
        svc: Arc::new(service),
    }
}

#[derive(Debug)]
pub struct ServiceFn<F, R> {
    f: F,
    _req: PhantomData<fn() -> R>,
}

impl<F, R, S> Service for ServiceFn<F, R>
where
    F: Fn(R) -> S,
    S: IntoFuture,
{
    type Request = R;
    type Response = S::Item;
    type Error = S::Error;
    type Future = S::Future;

    fn call(&self, req: Self::Request) -> Self::Future {
        (self.f)(req).into_future()
    }
}

#[derive(Debug)]
pub struct ConstService<S> {
    svc: Arc<S>,
}

impl<S> NewService for ConstService<S>
where
    S: Service,
{
    type Request = S::Request;
    type Response = S::Response;
    type Error = S::Error;
    type Instance = Arc<S>;

    fn new_service(&self) -> ::std::io::Result<Self::Instance> {
        Ok(self.svc.clone())
    }
}
