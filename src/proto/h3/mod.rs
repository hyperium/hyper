use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Buf;
use h3::server::Connection;

use pin_project_lite::pin_project;

mod glue;

pin_project! {
    pub(crate) struct Server<Q, S, B, E>
    where
        Q: crate::rt::quic::Connection<B>,
        B: Buf,
    {
        exec: E,
        q: Connection<glue::Conn<Q>, B>,
        s: S,
    }
}

impl<Q, S, B, E> Future for Server<Q, S, B, E>
where
    Q: crate::rt::quic::Connection<B>,
    B: Buf,
{
    type Output = crate::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        todo!()
    }
}
