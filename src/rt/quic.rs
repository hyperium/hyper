//! Generic QUIC support

use std::error::Error;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Buf;

// TODO: Should this be gated by an `http3` feature?

#[derive(Debug, Clone, Copy)]
pub struct ErrorCode(u64);

/// A QUIC connection.
pub trait Connection<B> {
    /// Send streams that can be opened by this connection.
    type SendStream: SendStream<B>;
    /// Receive streams that can be accepted by this connection.
    type RecvStream: RecvStream;
    /// Bidirectional streams that can be opened or accepted by this connection.
    type BidiStream: SendStream<B> + RecvStream;
    /// Errors that may occur opening or accepting streams.
    type Error: Error + Send + Sync + 'static;

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

    /// Close the stream
    // This is an async function because on closing, the QUIC protocol could send `reason`
    // to the other side of the connection(AFAIK, this should also immediatly close the connection
    // locally, not receiving the closing ACK from the other side, which I believe to be OK on the
    // protocol side of things).
    fn close(
        &mut self,
        // cx: &mut Context<'_>,
        code: u64,
        reason: &[u8],
    ) -> Result<(), Self::Error>;
}

// accepting name suggestions here
pub enum InitiatorSide {
    Client = 0,
    Server = 1,
}

// accepting name suggestions here
pub enum StreamDirection {
    Unidirectional = 1,
    Bidirectional = 0,
}

pub trait SendStreamID {
    fn index(&self) -> u64;
    fn initiator_side(&self) -> InitiatorSide;
    fn direction(&self) -> StreamDirection;
    /// the u62 number that identifies the stream
    fn u62_id(&self) -> u64;
}

/// The send portion of a QUIC stream.
pub trait SendStream<B> {
    /// Errors that may happen trying to send data.
    type Error: std::error::Error + Send + Sync + 'static;
    type SendStreamID: SendStreamID;
    /// Polls that the stream is ready to send more data.
    // Q: Should this be Pin<&mut Self>?
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>>;
    /// Send data on the stream.
    // Added another generic parameter because this was restricting the type of `Buf` to `B` in the
    // `SendStream<B>`, which I believe shouldn't be a restriction
    fn send_data<Buff: Buf>(&mut self, data: Buff) -> Result<(), Self::Error>;
    // fn poll_flush?
    /// finish?
    fn poll_finish(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>>;
    /// Close the stream with an error code.
    fn reset(&mut self, reset_code: u64);
    /// Get QUIC send stream id
    fn send_id(&self) -> Self::SendStreamID;
}

/// The receive portion of a QUIC stream.
pub trait RecvStream {
    /// Buffers of data that can be received.
    type Buf: Buf;
    /// Errors that may be received.
    type Error: std::error::Error + Send + Sync + 'static; // bounds?
    type SendStreamID: SendStreamID;

    // Q: should this be Pin?
    /// Poll for more data received from the remote on this stream.
    fn poll_data(&mut self, cx: &mut Context<'_>) -> Poll<Result<Option<Self::Buf>, Self::Error>>;
    /// Signal to the remote peer to stop sending data.
    fn stop_sending(&mut self, error_code: u64);
    /// Get QUIC send stream id
    fn recv_id(&self) -> Self::SendStreamID;
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
