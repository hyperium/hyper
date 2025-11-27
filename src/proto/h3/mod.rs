use std::future::Future;
use std::pin::{pin, Pin};
use std::task::{Context, Poll};

use bytes::Buf;
use futures_core::ready;
use futures_util::FutureExt;
use h3::server::Connection;

use pin_project_lite::pin_project;

mod glue;

pin_project! {
    pub(crate) struct Server<Q, S, B, E>
    where
        Q: crate::rt::quic::Connection<B>,
        Q: Unpin,
        Q: Clone,
        B: Buf,
    {
        exec: E,
        q: Connection<glue::Conn<Q>, B>,
        s: S,
    }
}

impl<Q, S, B, E> Server<Q, S, B, E>
where
    Q: crate::rt::quic::Connection<B> + Unpin + Clone,
    B: Buf,
{
    pub fn new(quic: Q, service: S) -> Self {
        todo!()
    }
}

impl<Q, S, B, E> Future for Server<Q, S, B, E>
where
    Q: crate::rt::quic::Connection<B> + Unpin + Clone,
    B: Buf,
{
    type Output = crate::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        use crate::error::Kind;
        loop {
            let accept_fut = self.q.accept();
            let Some(resolver) = ready!(pin!(accept_fut).poll(cx))
                .map_err(|err| crate::Error::new(Kind::Http3).with(err))?
            else {
                // I believe this means completed
                return Poll::Ready(Ok(()));
            };

            match ready!(pin!(resolver.resolve_request()).poll(cx)) {
                Ok((request, request_stream)) => {
                    // process request
                    // request.
                }
                Err(err) => {
                    // process request error
                }
            };
        }
    }
}
