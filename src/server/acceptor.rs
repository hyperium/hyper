use std::io;

use futures::{future, IntoFuture, Future};
use tokio_io::{AsyncRead, AsyncWrite};

pub trait Acceptor<Io> {
    type Item: AsyncRead + AsyncWrite;
    type Error: Into<Box<::std::error::Error + Send + Sync>>;
    type Accept: Future<Item = Self::Item, Error = Self::Error>;

    fn accept(&self, io: Io) -> Self::Accept;
}

impl<F, T, R> Acceptor<T> for F
where
    F: Fn(T) -> R,
    R: IntoFuture,
    R::Item: AsyncRead + AsyncWrite,
    R::Error: Into<Box<::std::error::Error + Send + Sync>>,
{
    type Item = R::Item;
    type Error = R::Error;
    type Accept = R::Future;

    #[inline]
    fn accept(&self, io: T) -> Self::Accept {
        (*self)(io).into_future()
    }
}

#[derive(Debug)]
pub struct Raw(());

impl Raw {
    pub(super) fn new() -> Raw {
        Raw(())
    }
}

impl<I> Acceptor<I> for Raw
where
    I: AsyncRead + AsyncWrite,
{
    type Item = I;
    type Error = io::Error;
    type Accept = future::FutureResult<Self::Item, Self::Error>;

    #[inline]
    fn accept(&self, io: I) -> Self::Accept {
        future::ok(io)
    }
}
