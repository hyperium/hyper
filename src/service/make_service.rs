use std::error::Error as StdError;
use std::fmt;

use futures::{Async, Future, IntoFuture, Poll};

use body::Payload;
use super::Service;

/// An asynchronous constructor of `Service`s.
pub trait MakeService<Ctx> {
    /// The `Payload` body of the `http::Request`.
    type ReqBody: Payload;

    /// The `Payload` body of the `http::Response`.
    type ResBody: Payload;

    /// The error type that can be returned by `Service`s.
    type Error: Into<Box<dyn StdError + Send + Sync>>;

    /// The resolved `Service` from `new_service()`.
    type Service: Service<
        ReqBody=Self::ReqBody,
        ResBody=Self::ResBody,
        Error=Self::Error,
    >;

    /// The future returned from `new_service` of a `Service`.
    type Future: Future<Item=Self::Service, Error=Self::MakeError>;

    /// The error type that can be returned when creating a new `Service`.
    type MakeError: Into<Box<dyn StdError + Send + Sync>>;

    /// Returns `Ready` when the constructor is ready to create a new `Service`.
    ///
    /// The implementation of this method is allowed to return a `Ready` even if
    /// the factory is not ready to create a new service. In this case, the future
    /// returned from `make_service` will resolve to an error.
    fn poll_ready(&mut self) -> Poll<(), Self::MakeError> {
        Ok(Async::Ready(()))
    }

    /// Create a new `Service`.
    fn make_service(&mut self, ctx: Ctx) -> Self::Future;
}

// Just a sort-of "trait alias" of `MakeService`, not to be implemented
// by anyone, only used as bounds.
#[doc(hidden)]
pub trait MakeServiceRef<Ctx>: self::sealed::Sealed<Ctx> {
    type ReqBody: Payload;
    type ResBody: Payload;
    type Error: Into<Box<dyn StdError + Send + Sync>>;
    type Service: Service<
        ReqBody=Self::ReqBody,
        ResBody=Self::ResBody,
        Error=Self::Error,
    >;
    type MakeError: Into<Box<dyn StdError + Send + Sync>>;
    type Future: Future<Item=Self::Service, Error=Self::MakeError>;

    // Acting like a #[non_exhaustive] for associated types of this trait.
    //
    // Basically, no one outside of hyper should be able to set this type
    // or declare bounds on it, so it should prevent people from creating
    // trait objects or otherwise writing code that requires using *all*
    // of the associated types.
    //
    // Why? So we can add new associated types to this alias in the future,
    // if necessary.
    type __DontNameMe: self::sealed::CantImpl;

    fn poll_ready_ref(&mut self) -> Poll<(), Self::MakeError>;

    fn make_service_ref(&mut self, ctx: &Ctx) -> Self::Future;
}

impl<T, Ctx, E, ME, S, F, IB, OB> MakeServiceRef<Ctx> for T
where
    T: for<'a> MakeService<&'a Ctx, Error=E, MakeError=ME, Service=S, Future=F, ReqBody=IB, ResBody=OB>,
    E: Into<Box<dyn StdError + Send + Sync>>,
    ME: Into<Box<dyn StdError + Send + Sync>>,
    S: Service<ReqBody=IB, ResBody=OB, Error=E>,
    F: Future<Item=S, Error=ME>,
    IB: Payload,
    OB: Payload,
{
    type Error = E;
    type Service = S;
    type ReqBody = IB;
    type ResBody = OB;
    type MakeError = ME;
    type Future = F;

    type __DontNameMe = self::sealed::CantName;

    fn poll_ready_ref(&mut self) -> Poll<(), Self::MakeError> {
        self.poll_ready()
    }

    fn make_service_ref(&mut self, ctx: &Ctx) -> Self::Future {
        self.make_service(ctx)
    }
}

impl<T, Ctx, E, ME, S, F, IB, OB> self::sealed::Sealed<Ctx> for T
where
    T: for<'a> MakeService<&'a Ctx, Error=E, MakeError=ME, Service=S, Future=F, ReqBody=IB, ResBody=OB>,
    E: Into<Box<dyn StdError + Send + Sync>>,
    ME: Into<Box<dyn StdError + Send + Sync>>,
    S: Service<ReqBody=IB, ResBody=OB, Error=E>,
    F: Future<Item=S, Error=ME>,
    IB: Payload,
    OB: Payload,
{}


/// Create a `MakeService` from a function.
///
/// # Example
///
/// ```rust,no_run
/// # #[cfg(feature = "runtime")] fn main() {
/// use std::net::TcpStream;
/// use hyper::{Body, Request, Response, Server};
/// use hyper::rt::{self, Future};
/// use hyper::server::conn::AddrStream;
/// use hyper::service::{make_service_fn, service_fn_ok};
///
/// let addr = ([127, 0, 0, 1], 3000).into();
///
/// let make_svc = make_service_fn(|socket: &AddrStream| {
///     let remote_addr = socket.remote_addr();
///     service_fn_ok(move |_: Request<Body>| {
///         Response::new(Body::from(format!("Hello, {}", remote_addr)))
///     })
/// });
///
/// // Then bind and serve...
/// let server = Server::bind(&addr)
///     .serve(make_svc);
///
/// // Finally, spawn `server` onto an Executor...
/// rt::run(server.map_err(|e| {
///     eprintln!("server error: {}", e);
/// }));
/// # }
/// # #[cfg(not(feature = "runtime"))] fn main() {}
/// ```
pub fn make_service_fn<F, Ctx, Ret>(f: F) -> MakeServiceFn<F>
where
    F: FnMut(&Ctx) -> Ret,
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
    F: FnMut(&Ctx) -> Ret,
    Ret: IntoFuture,
    Ret::Item: Service<ReqBody=ReqBody, ResBody=ResBody>,
    Ret::Error: Into<Box<dyn StdError + Send + Sync>>,
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

mod sealed {
    pub trait Sealed<T> {}

    pub trait CantImpl {}

    #[allow(missing_debug_implementations)]
    pub enum CantName {}

    impl CantImpl for CantName {}
}
