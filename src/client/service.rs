//! TODO: dox

use super::conn::{SendRequest, Builder};
use std::marker::PhantomData;
use crate::{common::{Poll, task, Pin}, body::Payload};
use std::future::Future;
use std::error::Error as StdError;
use tower_make::MakeConnection;

pub use tower_service::Service;
pub use tower_make::MakeService;

/// TODO: Dox
#[derive(Debug)]
pub struct Connect<C, B, T> {
    inner: C,
    builder: Builder,
    _pd: PhantomData<fn(T, B)>
}

impl<C, B, T> Connect<C, B, T> {
    /// TODO: dox
    pub fn new(inner: C, builder: Builder) -> Self {
        Self {
            inner,
            builder,
            _pd: PhantomData
        }
    }
}

impl<C, B, T> Service<T> for Connect<C, B, T>
where
    C: MakeConnection<T>,
    C::Connection: Unpin + Send + 'static,
    C::Future: Send + 'static,
    // TODO: this should not require the connection error to be send since
    // into box should handle this. I think.
    C::Error: Into<Box<dyn StdError + Send + Sync>> + Send,
    B: Payload + Unpin + 'static,
    B::Data: Unpin,
{
    type Response = SendRequest<B>;
    type Error = crate::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, cx: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: T) -> Self::Future {
        let builder = self.builder.clone();
        let io = self.inner.make_connection(req);

        let fut = async move {
            match io.await {
                Ok(io) => {
                    match builder.handshake(io).await {
                        Ok((sr, conn)) => {
                            crate::rt::spawn(async move {
                                if let Err(e) = conn.await {
                                    error!("connection error: {:?}", e);
                                }
                            });
                            Ok(sr)
                        },
                        Err(e) => Err(e)
                    }
                },
                Err(e) => {
                    let err = crate::Error::new(crate::error::Kind::Connect).with(e.into());
                    Err(err)
                }
            }
        };

        Box::pin(fut)
    }
}
