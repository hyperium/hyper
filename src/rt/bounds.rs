//! Trait aliases
//!
//! Traits in this module ease setting bounds and usually automatically
//! implemented by implementing another trait.

#[cfg(all(feature = "server", feature = "http2"))]
pub use self::h2::Http2ConnExec;

#[cfg(all(feature = "client", feature = "http2"))]
pub use self::h2_client::ExecutorClient;

#[cfg(all(feature = "client", feature = "http2"))]
#[cfg_attr(docsrs, doc(cfg(all(feature = "server", feature = "http2"))))]
mod h2_client {
    use std::{error::Error, future::Future};
    use tokio::io::{AsyncRead, AsyncWrite};

    use crate::{proto::h2::client::H2ClientFuture, rt::Executor};

    /// An executor to spawn http2 futures for the client.
    ///
    /// This trait is implemented for any type that implements [`Executor`]
    /// trait for any future.
    ///
    /// This trait is sealed and cannot be implemented for types outside this crate.
    ///
    /// [`Executor`]: crate::rt::Executor
    pub trait ExecutorClient<B, T>: sealed_client::Sealed<(B, T)>
    where
        B: http_body::Body,
        B::Error: Into<Box<dyn Error + Send + Sync>>,
        T: AsyncRead + AsyncWrite + Unpin,
    {
        #[doc(hidden)]
        fn execute_h2_future(&mut self, future: H2ClientFuture<B, T>);
    }

    impl<E, B, T> ExecutorClient<B, T> for E
    where
        E: Executor<H2ClientFuture<B, T>>,
        B: http_body::Body + 'static,
        B::Error: Into<Box<dyn Error + Send + Sync>>,
        H2ClientFuture<B, T>: Future<Output = ()>,
        T: AsyncRead + AsyncWrite + Unpin,
    {
        fn execute_h2_future(&mut self, future: H2ClientFuture<B, T>) {
            self.execute(future)
        }
    }

    impl<E, B, T> sealed_client::Sealed<(B, T)> for E
    where
        E: Executor<H2ClientFuture<B, T>>,
        B: http_body::Body + 'static,
        B::Error: Into<Box<dyn Error + Send + Sync>>,
        H2ClientFuture<B, T>: Future<Output = ()>,
        T: AsyncRead + AsyncWrite + Unpin,
    {
    }

    mod sealed_client {
        pub trait Sealed<X> {}
    }
}

#[cfg(all(feature = "server", feature = "http2"))]
#[cfg_attr(docsrs, doc(cfg(all(feature = "server", feature = "http2"))))]
mod h2 {
    use crate::{proto::h2::server::H2Stream, rt::Executor};
    use http_body::Body;
    use std::future::Future;

    /// An executor to spawn http2 connections.
    ///
    /// This trait is implemented for any type that implements [`Executor`]
    /// trait for any future.
    ///
    /// This trait is sealed and cannot be implemented for types outside this crate.
    ///
    /// [`Executor`]: crate::rt::Executor
    pub trait Http2ConnExec<F, B: Body>: sealed::Sealed<(F, B)> + Clone {
        #[doc(hidden)]
        fn execute_h2stream(&mut self, fut: H2Stream<F, B>);
    }

    #[doc(hidden)]
    impl<E, F, B> Http2ConnExec<F, B> for E
    where
        E: Executor<H2Stream<F, B>> + Clone,
        H2Stream<F, B>: Future<Output = ()>,
        B: Body,
    {
        fn execute_h2stream(&mut self, fut: H2Stream<F, B>) {
            self.execute(fut)
        }
    }

    impl<E, F, B> sealed::Sealed<(F, B)> for E
    where
        E: Executor<H2Stream<F, B>> + Clone,
        H2Stream<F, B>: Future<Output = ()>,
        B: Body,
    {
    }

    mod sealed {
        pub trait Sealed<T> {}
    }
}
