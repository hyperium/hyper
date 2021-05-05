use std::convert::Infallible;
use std::error::Error as StdError;
use std::fmt;

use tokio::io::{AsyncRead, AsyncWrite};

use pin_project::pin_project;

use super::{HttpService, Service};
use crate::body::HttpBody;
use crate::common::{task, Future, Pin, Poll};

// The same "trait alias" as tower::MakeConnection, but inlined to reduce
// dependencies.
pub trait MakeConnection<Target>: self::sealed::Sealed<(Target,)> {
    type Connection: AsyncRead + AsyncWrite;
    type Error;
    type Future: Future<Output = Result<Self::Connection, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>>;
    fn make_connection(&mut self, target: Target) -> Self::Future;
}

impl<S, Target> self::sealed::Sealed<(Target,)> for S where S: Service<Target> {}

impl<S, Target> MakeConnection<Target> for S
where
    S: Service<Target>,
    S::Response: AsyncRead + AsyncWrite,
{
    type Connection = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Service::poll_ready(self, cx)
    }

    fn make_connection(&mut self, target: Target) -> Self::Future {
        Service::call(self, target)
    }
}

// Just a sort-of "trait alias" of `MakeService`, not to be implemented
// by anyone, only used as bounds.
pub trait MakeServiceRef<Target, ReqBody>: self::sealed::Sealed<(Target, ReqBody)> {
    type ResBody: HttpBody;
    type Error: Into<Box<dyn StdError + Send + Sync>>;
    type Service: HttpService<ReqBody, ResBody = Self::ResBody, Error = Self::Error>;
    type MakeError: Into<Box<dyn StdError + Send + Sync>>;
    type Future: Future<Output = Result<Self::Service, Self::MakeError>>;

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
    T: for<'a> Service<&'a Target, Error = ME, Response = S, Future = F>,
    E: Into<Box<dyn StdError + Send + Sync>>,
    ME: Into<Box<dyn StdError + Send + Sync>>,
    S: HttpService<IB, ResBody = OB, Error = E>,
    F: Future<Output = Result<S, ME>>,
    IB: HttpBody,
    OB: HttpBody,
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

impl<T, Target, S, B1, B2> self::sealed::Sealed<(Target, B1)> for T
where
    T: for<'a> Service<&'a Target, Response = S>,
    S: HttpService<B1, ResBody = B2>,
    B1: HttpBody,
    B2: HttpBody,
{
}

/// Create a `MakeService` from a function.
///
/// # Example
///
/// ```
/// # #[cfg(feature = "runtime")]
/// # async fn run() {
/// use std::convert::Infallible;
/// use hyper::{Body, Request, Response, Server};
/// use hyper::server::conn::AddrStream;
/// use hyper::service::{make_service_fn, service_fn};
///
/// let addr = ([127, 0, 0, 1], 3000).into();
///
/// let make_svc = make_service_fn(|socket: &AddrStream| {
///     let remote_addr = socket.remote_addr();
///     async move {
///         Ok::<_, Infallible>(service_fn(move |_: Request<Body>| async move {
///             Ok::<_, Infallible>(
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
/// # fn main() {}
/// ```
pub fn make_service_fn<F, Target, Ret>(f: F) -> MakeServiceFn<F>
where
    F: FnMut(&Target) -> Ret,
    Ret: Future,
{
    MakeServiceFn { f }
}

/// `MakeService` returned from [`make_service_fn`]
#[derive(Clone, Copy)]
pub struct MakeServiceFn<F> {
    f: F,
}

impl<'t, F, Ret, Target, Svc, MkErr> Service<&'t Target> for MakeServiceFn<F>
where
    F: FnMut(&Target) -> Ret,
    Ret: Future<Output = Result<Svc, MkErr>>,
    MkErr: Into<Box<dyn StdError + Send + Sync>>,
{
    type Error = MkErr;
    type Response = Svc;
    type Future = Ret;

    fn poll_ready(&mut self, _cx: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, target: &'t Target) -> Self::Future {
        (self.f)(target)
    }
}

impl<F> fmt::Debug for MakeServiceFn<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MakeServiceFn").finish()
    }
}

/// A `MakeService` that produces services by cloning an inner service.
#[derive(Debug, Clone, Copy)]
pub struct Shared<S> {
    svc: S,
}

impl<S> Shared<S> {
    pub(crate) fn new(svc: S) -> Self {
        Self { svc }
    }
}

impl<T, S> Service<T> for Shared<S>
where
    S: Clone,
{
    type Error = Infallible;
    type Response = S;
    type Future = SharedFuture<S>;

    fn poll_ready(&mut self, _cx: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _target: T) -> Self::Future {
        SharedFuture(futures_util::future::ok(self.svc.clone()))
    }
}

/// Response future for [`Shared`].
#[pin_project]
#[derive(Debug)]
pub struct SharedFuture<S>(#[pin] futures_util::future::Ready<Result<S, Infallible>>);

impl<S> Future for SharedFuture<S> {
    type Output = Result<S, Infallible>;

    fn poll(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        self.project().0.poll(cx)
    }
}

mod sealed {
    pub trait Sealed<X> {}

    #[allow(unreachable_pub)] // This is intentional.
    pub trait CantImpl {}

    #[allow(missing_debug_implementations)]
    pub enum CantName {}

    impl CantImpl for CantName {}
}
