use std::error::Error as StdError;
use std::fmt;

use crate::body::Payload;
use crate::common::{Future, Poll, task};
use super::Service;

/// An asynchronous constructor of `Service`s.
pub trait MakeService<Target, ReqBody>: sealed::Sealed<Target, ReqBody> {
    /// The `Payload` body of the `http::Response`.
    type ResBody: Payload;

    /// The error type that can be returned by `Service`s.
    type Error: Into<Box<dyn StdError + Send + Sync>>;

    /// The resolved `Service` from `new_service()`.
    type Service: Service<
        ReqBody,
        ResBody=Self::ResBody,
        Error=Self::Error,
    >;

    /// The future returned from `new_service` of a `Service`.
    type Future: Future<Output=Result<Self::Service, Self::MakeError>>;

    /// The error type that can be returned when creating a new `Service`.
    type MakeError: Into<Box<dyn StdError + Send + Sync>>;

    /// Returns `Ready` when the constructor is ready to create a new `Service`.
    ///
    /// The implementation of this method is allowed to return a `Ready` even if
    /// the factory is not ready to create a new service. In this case, the future
    /// returned from `make_service` will resolve to an error.
    fn poll_ready(&mut self, cx: &mut task::Context<'_>) -> Poll<Result<(), Self::MakeError>> {
        Poll::Ready(Ok(()))
    }

    /// Create a new `Service`.
    fn make_service(&mut self, target: Target) -> Self::Future;
}

impl<T, Target, S, B1, B2, E, F> MakeService<Target, B1> for T 
where 
    T: for<'a> tower_service::Service<&'a Target, Response = S, Error = E, Future = F>,
    S: tower_service::Service<crate::Request<B1>, Response = crate::Response<B2>>,
    E: Into<Box<dyn std::error::Error + Send + Sync>>,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    B1: Payload,
    B2: Payload,
    F: Future<Output = Result<S, E>>,
{
    type ResBody = B2;
    type Error = S::Error;
    type Service = S;
    type Future = F;
    type MakeError = E;

    fn poll_ready(&mut self, cx: &mut task::Context<'_>) -> Poll<Result<(), Self::MakeError>> {
         tower_service::Service::poll_ready(self, cx)
    }

    fn make_service(&mut self, req: Target) -> Self::Future {
        tower_service::Service::call(self, &req)
    }
}

impl<T, Target, S, B1, B2> sealed::Sealed<Target, B1> for T 
where 
    T: for<'a> tower_service::Service<&'a Target, Response = S>,
    S: tower_service::Service<crate::Request<B1>, Response = crate::Response<B2>>
{
}

// Just a sort-of "trait alias" of `MakeService`, not to be implemented
// by anyone, only used as bounds.
#[doc(hidden)]
pub trait MakeServiceRef<Target, ReqBody>: self::sealed::Sealed<Target, ReqBody> {
    type ResBody: Payload;
    type Error: Into<Box<dyn StdError + Send + Sync>>;
    type Service: Service<
        ReqBody,
        ResBody=Self::ResBody,
        Error=Self::Error,
    >;
    type MakeError: Into<Box<dyn StdError + Send + Sync>>;
    type Future: Future<Output=Result<Self::Service, Self::MakeError>>;

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

    fn poll_ready_ref(&mut self, cx: &mut task::Context<'_>) -> Poll<Result<(), Self::MakeError>>;

    fn make_service_ref(&mut self, target: &Target) -> Self::Future;
}

impl<T, Target, E, ME, S, F, IB, OB> MakeServiceRef<Target, IB> for T
where
    T: for<'a> tower_service::Service<&'a Target, Error=ME, Response=S, Future=F>,
    E: Into<Box<dyn StdError + Send + Sync>>,
    ME: Into<Box<dyn StdError + Send + Sync>>,
    S: tower_service::Service<crate::Request<IB>, Response=crate::Response<OB>, Error=E>,
    F: Future<Output=Result<S, ME>>,
    IB: Payload,
    OB: Payload,
{
    type Error = E;
    type Service = S;
    type ResBody = OB;
    type MakeError = ME;
    type Future = F;

    type __DontNameMe = self::sealed::CantName;

    fn poll_ready_ref(&mut self, cx: &mut task::Context<'_>) -> Poll<Result<(), Self::MakeError>> {
        self.poll_ready(cx)
    }

    fn make_service_ref(&mut self, target: &Target) -> Self::Future {
        self.call(target)
    }
}

/// Create a `MakeService` from a function.
///
/// # Example
///
/// ```rust,no_run
/// # #[cfg(feature = "runtime")]
/// # #[tokio::main]
/// # async fn main() {
/// use std::net::TcpStream;
/// use hyper::{Body, Error, Request, Response, Server};
/// use hyper::rt::{self, Future};
/// use hyper::server::conn::AddrStream;
/// use hyper::service::{make_service_fn, service_fn};
///
/// let addr = ([127, 0, 0, 1], 3000).into();
///
/// let make_svc = make_service_fn(|socket: &AddrStream| {
///     let remote_addr = socket.remote_addr();
///     async move {
///         Ok::<_, Error>(service_fn(move |_: Request<Body>| async move {
///             Ok::<_, Error>(
///                 Response::new(Body::from(format!("Hello, {}!", remote_addr)))
///             )
///         }))
///     }
/// });
///
/// // Then bind and serve...
/// let server = Server::bind(&addr)
///     .serve(make_svc);
///
/// // Finally, spawn `server` onto an Executor...
/// if let Err(e) = server.await {
///     eprintln!("server error: {}", e);
/// }
/// # }
/// # #[cfg(not(feature = "runtime"))] fn main() {}
/// ```
pub fn make_service_fn<F, Target, Ret>(f: F) -> MakeServiceFn<F>
where
    F: FnMut(&Target) -> Ret,
    Ret: Future,
{
    MakeServiceFn {
        f,
    }
}

// Not exported from crate as this will likely be replaced with `impl Service`.
pub struct MakeServiceFn<F> {
    f: F,
}

impl<'t, F, Ret, Target, Svc, MkErr> tower_service::Service<&'t Target> for MakeServiceFn<F>
where
    F: FnMut(&Target) -> Ret,
    Ret: Future<Output=Result<Svc, MkErr>>,
    MkErr: Into<Box<dyn StdError + Send + Sync>>,
{
    type Error = MkErr;
    type Response = Svc;
    type Future = Ret;

    fn poll_ready(&mut self, cx: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, target: &'t Target) -> Self::Future {
        (self.f)(target)
    }
}

impl<F> fmt::Debug for MakeServiceFn<F> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("MakeServiceFn")
            .finish()
    }
}

mod sealed {
    pub trait Sealed<T, B> {}

    pub trait CantImpl {}

    #[allow(missing_debug_implementations)]
    pub enum CantName {}

    impl CantImpl for CantName {}
}
