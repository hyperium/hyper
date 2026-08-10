use std::task::{Context, Poll};

use bytes::Buf;

pub(super) struct Conn<Q>(Q);

pub(super) struct BidiStream<S>(S);
pub(super) struct SendStream<S>(S);
pub(super) struct RecvStream<S>(S);

impl<Q, B> h3::quic::Connection<B> for Conn<Q>
where
    Q: crate::rt::quic::Connection<B>,
    B: Buf,
{
    type RecvStream = RecvStream<Q::RecvStream>;
    type OpenStreams = Self;

    fn poll_accept_recv(&mut self, _cx: &mut Context<'_>)
        -> Poll<Result<Self::RecvStream, h3::quic::ConnectionErrorIncoming>>
    {
        todo!();
    }

    fn poll_accept_bidi(&mut self, _cx: &mut Context<'_>)
        -> Poll<Result<Self::BidiStream, h3::quic::ConnectionErrorIncoming>>
    {
        todo!();
    }

    fn opener(&self) -> Self::OpenStreams {
        todo!();
    }
}

impl<Q, B> h3::quic::OpenStreams<B> for Conn<Q>
where
    Q: crate::rt::quic::Connection<B>,
    B: Buf,
{
    type BidiStream = BidiStream<Q::BidiStream>;
    type SendStream = SendStream<Q::SendStream>;

    fn poll_open_send(&mut self, _cx: &mut Context<'_>)
        -> Poll<Result<Self::SendStream, h3::quic::StreamErrorIncoming>>
    {
        todo!();
    }

    fn poll_open_bidi(&mut self, _cx: &mut Context<'_>)
        -> Poll<Result<Self::BidiStream, h3::quic::StreamErrorIncoming>>
    {
        todo!();
    }

    fn close(&mut self, _: h3::error::Code, _: &[u8]) {

    }
}

impl<S, B> h3::quic::SendStream<B> for BidiStream<S>
where
    S: crate::rt::quic::SendStream<B>,
    B: Buf,
{
    // Required methods
    fn poll_ready(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), h3::quic::StreamErrorIncoming>> {
        todo!();
    }
    fn send_data<T: Into<h3::quic::WriteBuf<B>>>(
        &mut self,
        data: T,
    ) -> Result<(), h3::quic::StreamErrorIncoming> {
        todo!();
    }
    fn poll_finish(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), h3::quic::StreamErrorIncoming>> {
        todo!();
    }
    fn reset(&mut self, reset_code: u64) {
        todo!();
    }
    fn send_id(&self) -> h3::quic::StreamId {
        todo!()
    }
}

impl<S, B> h3::quic::SendStream<B> for SendStream<S>
where
    S: crate::rt::quic::SendStream<B>,
    B: Buf,
{
    // Required methods
    fn poll_ready(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), h3::quic::StreamErrorIncoming>> {
        todo!();
    }
    fn send_data<T: Into<h3::quic::WriteBuf<B>>>(
        &mut self,
        data: T,
    ) -> Result<(), h3::quic::StreamErrorIncoming> {
        todo!();
    }
    fn poll_finish(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), h3::quic::StreamErrorIncoming>> {
        todo!();
    }
    fn reset(&mut self, reset_code: u64) {
        todo!();
    }
    fn send_id(&self) -> h3::quic::StreamId {
        todo!()
    }
}

impl<S> h3::quic::RecvStream for BidiStream<S>
where
    S: crate::rt::quic::RecvStream,
{
    type Buf = S::Buf;

    // Required methods
    fn poll_data(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Option<Self::Buf>, h3::quic::StreamErrorIncoming>> {
        todo!();
    }
    fn stop_sending(&mut self, error_code: u64) {
        todo!();
    }
    fn recv_id(&self) -> h3::quic::StreamId {
        todo!();
    }
}

impl<S> h3::quic::RecvStream for RecvStream<S>
where
    S: crate::rt::quic::RecvStream,
{
    type Buf = S::Buf;

    // Required methods
    fn poll_data(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Option<Self::Buf>, h3::quic::StreamErrorIncoming>> {
        todo!();
    }
    fn stop_sending(&mut self, error_code: u64) {
        todo!();
    }
    fn recv_id(&self) -> h3::quic::StreamId {
        todo!();
    }
}
