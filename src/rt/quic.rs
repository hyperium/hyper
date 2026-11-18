//! Generic QUIC support

use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Buf;

// TODO: Should this be gated by an `http3` feature?

/// A QUIC connection.
pub trait Connection<B> {
    /// Send streams that can be opened by this connection.
    type SendStream: SendStream<B>;
    /// Receive streams that can be accepted by this connection.
    type RecvStream: RecvStream;
    /// Bidirectional streams that can be opened or accepted by this connection.
    type BidiStream: SendStream<B> + RecvStream;
    /// Errors that may occur opening or accepting streams.
    type Error;

    // Accepting streams

    // Q: shorten to bidi?
    /// Accept a bidirection stream.
    fn poll_accept_bidirectional_stream(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Option<Self::BidiStream>, Self::Error>>;

    /// Accept a unidirectional receive stream.
    fn poll_accept_recv_stream(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Option<Self::RecvStream>, Self::Error>>;

    // Creating streams

    // Q: shorten to bidi?
    /// Open a bidirectional stream.
    fn poll_open_bidirectional_stream(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::BidiStream, Self::Error>>;

    /// Open a unidirectional send stream.
    fn poll_open_send_stream(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::SendStream, Self::Error>>;
}

/// The send portion of a QUIC stream.
pub trait SendStream<B> {
    /// Errors that may happen trying to send data.
    type Error; // bounds?
    /// Polls that the stream is ready to send more data.
    // Q: Should this be Pin<&mut Self>?
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>>;
    /// Send data on the stream.
    fn send_data(&mut self, data: B) -> Result<(), Self::Error>;
    // fn poll_flush?
    /// finish?
    fn poll_finish(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>>;
    /// Close the stream with an error code.
    fn reset(&mut self, reset_code: u64);
}

/// The receive portion of a QUIC stream.
pub trait RecvStream {
    /// Buffers of data that can be received.
    type Buf: Buf;
    /// Errors that may be received.
    type Error; // bounds?

    // Q: should this be Pin?
    /// Poll for more data received from the remote on this stream.
    fn poll_data(&mut self, cx: &mut Context<'_>) -> Poll<Result<Option<Self::Buf>, Self::Error>>;
    /// Signal to the remote peer to stop sending data.
    fn stop_sending(&mut self, error_code: u64);
}

/// An optional trait if a QUIC stream can be split into two sides.
pub trait BidiStream<B>: SendStream<B> + RecvStream {
    /// The send side of a stream.
    type SendStream: SendStream<B>;
    /// The receive side of a stream.
    type RecvStream: RecvStream;

    /// Split this stream into two sides.
    fn split(self) -> (Self::SendStream, Self::RecvStream);
}
