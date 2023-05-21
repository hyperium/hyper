use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncWrite};

use crate::proto::h2::client::H2ClientFuture;
use crate::rt::Executor;

pub(crate) type BoxSendFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

// Executor must be provided by the user
#[derive(Clone)]
pub(crate) struct Exec(Arc<dyn Executor<BoxSendFuture> + Send + Sync>);

// ===== impl Exec =====

impl Exec {
    pub(crate) fn new<E>(exec: E) -> Self
    where
        E: Executor<BoxSendFuture> + Send + Sync + 'static,
    {
        Self(Arc::new(exec))
    }

    pub(crate) fn execute<F>(&self, fut: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.0.execute(Box::pin(fut))
    }
}

impl fmt::Debug for Exec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Exec").finish()
    }
}

pub trait ExecutorClient<B, T>
where
    B: http_body::Body,
    B::Error: std::error::Error + Send + Sync + 'static,
    T: AsyncRead + AsyncWrite + Unpin,
{
    fn execute_h2_future(&mut self, future: H2ClientFuture<B, T>);
}

impl<E, B, T> ExecutorClient<B, T> for E
where
    E: Executor<H2ClientFuture<B, T>>,
    B: http_body::Body + 'static,
    B::Error: std::error::Error + Send + Sync + 'static,
    H2ClientFuture<B, T>: Future<Output = ()>,
    T: AsyncRead + AsyncWrite + Unpin,
{
    fn execute_h2_future(&mut self, future: H2ClientFuture<B, T>) {
        self.execute(future)
    }
}

// If http2 is not enable, we just have a stub here, so that the trait bounds
// that *would* have been needed are still checked. Why?
//
// Because enabling `http2` shouldn't suddenly add new trait bounds that cause
// a compilation error.
#[cfg(not(feature = "http2"))]
#[allow(missing_debug_implementations)]
pub struct H2Stream<F, B>(std::marker::PhantomData<(F, B)>);

#[cfg(not(feature = "http2"))]
impl<F, B, E> Future for H2Stream<F, B>
where
    F: Future<Output = Result<http::Response<B>, E>>,
    B: crate::body::Body,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    E: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    type Output = ();

    fn poll(
        self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        unreachable!()
    }
}
