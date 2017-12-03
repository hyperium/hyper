use std::marker::PhantomData;
use std::sync::Arc;

use futures::IntoFuture;
use tokio_service::{NewService, Service};

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
