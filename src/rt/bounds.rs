//! Trait aliases
//!
//! Traits in this module ease setting bounds and usually automatically
//! implemented by implementing another trait.

#[cfg(all(feature = "server", feature = "http2"))]
pub use self::h2::Http2ConnExec;

#[cfg(all(feature = "server", feature = "http2"))]
#[cfg_attr(docsrs, doc(cfg(all(feature = "server", feature = "http2"))))]
mod h2 {
    use crate::{common::exec::Exec, proto::h2::server::H2Stream, rt::Executor};
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

    impl<F, B> Http2ConnExec<F, B> for Exec
    where
        H2Stream<F, B>: Future<Output = ()> + Send + 'static,
        B: Body,
    {
        fn execute_h2stream(&mut self, fut: H2Stream<F, B>) {
            self.execute(fut)
        }
    }

    impl<F, B> sealed::Sealed<(F, B)> for Exec
    where
        H2Stream<F, B>: Future<Output = ()> + Send + 'static,
        B: Body,
    {
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
