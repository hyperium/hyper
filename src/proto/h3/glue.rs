use std::{
    fmt::Display,
    task::{Context, Poll},
};

use bytes::Buf;
use futures_core::ready;

pub(super) struct Conn<Q>(Q);

impl<Q> Clone for Conn<Q>
where
    Q: Clone,
{
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

pub(super) struct BidiStream<S>(S);
pub(super) struct SendStream<S>(S);
pub(super) struct RecvStream<S>(S);

impl<Q, B> h3::quic::Connection<B> for Conn<Q>
where
    Q: crate::rt::quic::Connection<B> + Unpin + Clone,
    B: Buf,
{
    type RecvStream = RecvStream<Q::RecvStream>;
    type OpenStreams = Self;

    fn poll_accept_recv(
        &mut self,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<Self::RecvStream, h3::quic::ConnectionErrorIncoming>> {
        let pinned_self = ::std::pin::Pin::new(&mut self.0);

        let recv_stream = match ready!(pinned_self.poll_accept_recv_stream(_cx)) {
            Ok(Some(recv_stream)) => recv_stream,
            Ok(None) => {
                return Poll::Ready(Err(h3::quic::ConnectionErrorIncoming::Undefined(
                    ::std::sync::Arc::new(GlueError::<Q::Error> {
                        description: String::from("Error accepting receive stream"),
                        source: None,
                    }),
                )));
            }
            Err(err) => {
                return Poll::Ready(Err(h3::quic::ConnectionErrorIncoming::Undefined(
                    ::std::sync::Arc::new(GlueError {
                        description: String::from("Error accepting receive stream"),
                        source: Some(err),
                    }),
                )));
            }
        };

        Poll::Ready(Ok(RecvStream(recv_stream)))
    }

    fn poll_accept_bidi(
        &mut self,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<Self::BidiStream, h3::quic::ConnectionErrorIncoming>> {
        let pinned_self = ::std::pin::Pin::new(&mut self.0);

        let bidi_stream = match ready!(pinned_self.poll_accept_bidirectional_stream(_cx)) {
            Ok(Some(bidi_stream)) => bidi_stream,
            Ok(None) => {
                return Poll::Ready(Err(h3::quic::ConnectionErrorIncoming::Undefined(
                    ::std::sync::Arc::new(GlueError::<Q::Error> {
                        description: String::from("Error accepting receive stream"),
                        source: None,
                    }),
                )));
            }
            Err(err) => {
                return Poll::Ready(Err(h3::quic::ConnectionErrorIncoming::Undefined(
                    ::std::sync::Arc::new(GlueError {
                        description: String::from("Error accepting receive stream"),
                        source: Some(err),
                    }),
                )));
            }
        };

        Poll::Ready(Ok(BidiStream(bidi_stream)))
    }

    fn opener(&self) -> Self::OpenStreams {
        self.clone()
    }
}

impl<Q, B> h3::quic::OpenStreams<B> for Conn<Q>
where
    Q: crate::rt::quic::Connection<B> + Unpin,
    B: Buf,
{
    type BidiStream = BidiStream<Q::BidiStream>;
    type SendStream = SendStream<Q::SendStream>;

    fn poll_open_send(
        &mut self,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<Self::SendStream, h3::quic::StreamErrorIncoming>> {
        let pinned_self = ::std::pin::Pin::new(&mut self.0);

        let send_stream = match ready!(pinned_self.poll_open_send_stream(_cx)) {
            Ok(send_stream) => send_stream,
            Err(err) => {
                return Poll::Ready(Err(h3::quic::StreamErrorIncoming::Unknown(Box::new(
                    GlueError {
                        description: String::from("Error accepting receive stream"),
                        source: Some(err),
                    },
                ))));
            }
        };

        Poll::Ready(Ok(SendStream(send_stream)))
    }

    fn poll_open_bidi(
        &mut self,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<Self::BidiStream, h3::quic::StreamErrorIncoming>> {
        let pinned_self = ::std::pin::Pin::new(&mut self.0);

        let bidi_stream = match ready!(pinned_self.poll_open_bidirectional_stream(_cx)) {
            Ok(bidi_stream) => bidi_stream,
            Err(err) => {
                return Poll::Ready(Err(h3::quic::StreamErrorIncoming::Unknown(Box::new(
                    GlueError {
                        description: String::from("Error accepting receive stream"),
                        source: Some(err),
                    },
                ))));
            }
        };

        Poll::Ready(Ok(BidiStream(bidi_stream)))
    }

    fn close(&mut self, code: h3::error::Code, reason: &[u8]) {
        self.0.close(code.value(), reason);
    }
}

impl<S> BidiStream<S> {
    fn send_data<Buff, B>(
        &mut self,
        data: h3::quic::WriteBuf<Buff>,
    ) -> Result<(), h3::quic::StreamErrorIncoming>
    where
        S: crate::rt::quic::SendStream<B>,
        B: Buf,
        Buff: Buf,
    {
        use crate::rt::quic::SendStream as SendStreamTrait;

        match <S as SendStreamTrait<B>>::send_data(&mut self.0, data) {
            Ok(()) => Ok(()),
            Err(err) => {
                return Err(h3::quic::StreamErrorIncoming::Unknown(Box::new(
                    GlueError {
                        description: String::from("Sending data to the bidirectional stream"),
                        source: Some(err),
                    },
                )));
            }
        }
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
        let ready = match ready!(self.0.poll_ready(cx)) {
            Ok(ready) => ready,
            Err(err) => {
                return Poll::Ready(Err(h3::quic::StreamErrorIncoming::Unknown(Box::new(
                    GlueError {
                        description: String::from("Error accepting receive stream"),
                        source: Some(err),
                    },
                ))));
            }
        };

        Poll::Ready(Ok(ready))
    }
    fn send_data<T: Into<h3::quic::WriteBuf<B>>>(
        &mut self,
        data: T,
    ) -> Result<(), h3::quic::StreamErrorIncoming> {
        self.send_data(data.into())
    }
    fn poll_finish(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), h3::quic::StreamErrorIncoming>> {
        let done = match ready!(self.0.poll_finish(cx)) {
            Ok(done) => done,
            Err(err) => {
                return Poll::Ready(Err(h3::quic::StreamErrorIncoming::Unknown(Box::new(
                    GlueError {
                        description: String::from("Getting finish state"),
                        source: Some(err),
                    },
                ))));
            }
        };

        Poll::Ready(Ok(done))
    }
    fn reset(&mut self, reset_code: u64) {
        self.0.reset(reset_code);
    }
    fn send_id(&self) -> h3::quic::StreamId {
        match crate::rt::quic::SendStreamID::u62_id(&self.0.send_id()).try_into() {
            Ok(id) => id,
            Err(err) => {
                // As there is no room for error in the API, this is the first solution that came
                // to mind. Reconstructing the number from the other values, could result in the
                // same place so this implementation seems cleaner to me
                panic!("Invalid u62 QUIC stream ID: {err}");
            }
        }
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
        let ready = match ready!(self.0.poll_ready(cx)) {
            Ok(ready) => ready,
            Err(err) => {
                return Poll::Ready(Err(h3::quic::StreamErrorIncoming::Unknown(Box::new(
                    GlueError {
                        description: String::from("Error accepting receive stream"),
                        source: Some(err),
                    },
                ))));
            }
        };

        Poll::Ready(Ok(ready))
    }
    fn send_data<T: Into<h3::quic::WriteBuf<B>>>(
        &mut self,
        data: T,
    ) -> Result<(), h3::quic::StreamErrorIncoming> {
        use crate::rt::quic::SendStream as SendStreamTrait;

        match <S as SendStreamTrait<B>>::send_data(&mut self.0, data.into()) {
            Ok(()) => Ok(()),
            Err(err) => {
                return Err(h3::quic::StreamErrorIncoming::Unknown(Box::new(
                    GlueError {
                        description: String::from("Sending data to the bidirectional stream"),
                        source: Some(err),
                    },
                )));
            }
        }
    }
    fn poll_finish(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), h3::quic::StreamErrorIncoming>> {
        let done = match ready!(self.0.poll_finish(cx)) {
            Ok(done) => done,
            Err(err) => {
                return Poll::Ready(Err(h3::quic::StreamErrorIncoming::Unknown(Box::new(
                    GlueError {
                        description: String::from("Getting finish state"),
                        source: Some(err),
                    },
                ))));
            }
        };

        Poll::Ready(Ok(done))
    }
    fn reset(&mut self, reset_code: u64) {
        self.0.reset(reset_code);
    }
    fn send_id(&self) -> h3::quic::StreamId {
        match crate::rt::quic::SendStreamID::u62_id(&self.0.send_id()).try_into() {
            Ok(id) => id,
            Err(err) => {
                // As there is no room for error in the API, this is the first solution that came
                // to mind. Reconstructing the number from the other values, could result in the
                // same place so this implementation seems cleaner to me
                panic!("Invalid u62 QUIC stream ID: {err}");
            }
        }
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
        let ready = match ready!(self.0.poll_data(cx)) {
            Ok(ready) => ready,
            Err(err) => {
                return Poll::Ready(Err(h3::quic::StreamErrorIncoming::Unknown(Box::new(
                    GlueError {
                        description: String::from("Error accepting receive stream"),
                        source: Some(err),
                    },
                ))));
            }
        };

        Poll::Ready(Ok(ready))
    }
    fn stop_sending(&mut self, error_code: u64) {
        self.0.stop_sending(error_code);
    }
    fn recv_id(&self) -> h3::quic::StreamId {
        match crate::rt::quic::SendStreamID::u62_id(&self.0.recv_id()).try_into() {
            Ok(id) => id,
            Err(err) => {
                // As there is no room for error in the API, this is the first solution that came
                // to mind. Reconstructing the number from the other values, could result in the
                // same place so this implementation seems cleaner to me
                panic!("Invalid u62 QUIC stream ID: {err}");
            }
        }
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
        let ready = match ready!(self.0.poll_data(cx)) {
            Ok(ready) => ready,
            Err(err) => {
                return Poll::Ready(Err(h3::quic::StreamErrorIncoming::Unknown(Box::new(
                    GlueError {
                        description: String::from("Error accepting receive stream"),
                        source: Some(err),
                    },
                ))));
            }
        };

        Poll::Ready(Ok(ready))
    }
    fn stop_sending(&mut self, error_code: u64) {
        self.0.stop_sending(error_code);
    }
    fn recv_id(&self) -> h3::quic::StreamId {
        match crate::rt::quic::SendStreamID::u62_id(&self.0.recv_id()).try_into() {
            Ok(id) => id,
            Err(err) => {
                // As there is no room for error in the API, this is the first solution that came
                // to mind. Reconstructing the number from the other values, could result in the
                // same place so this implementation seems cleaner to me
                panic!("Invalid u62 QUIC stream ID: {err}");
            }
        }
    }
}

#[derive(Debug)]
pub(super) struct GlueError<E> {
    description: String,
    source: Option<E>,
}

impl<E: std::error::Error + 'static> std::error::Error for GlueError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let Some(ref value) = self.source {
            return Some(value as &(dyn std::error::Error + 'static));
        }

        None
    }

    fn description(&self) -> &str {
        self.description.as_str()
    }

    fn cause(&self) -> Option<&dyn std::error::Error> {
        self.source()
    }
}

impl<E: Display> Display for GlueError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ConnectionError{{ description: {}, source: {} }}",
            self.description,
            if let Some(ref src) = self.source {
                format!("{src}")
            } else {
                "None".to_string()
            }
        )
    }
}
