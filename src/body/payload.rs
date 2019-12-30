use std::error::Error as StdError;

use bytes::Buf;
use http::HeaderMap;

use crate::common::{task, Pin, Poll};
use http_body::{Body as HttpBody, SizeHint};

/// This trait represents a streaming body of a `Request` or `Response`.
///
/// The built-in implementation of this trait is [`Body`](::Body), in case you
/// don't need to customize a send stream for your own application.
pub trait Payload: sealed::Sealed + Send + 'static {
    /// A buffer of bytes representing a single chunk of a body.
    type Data: Buf + Send;

    /// The error type of this stream.
    type Error: Into<Box<dyn StdError + Send + Sync>>;

    /// Poll for a `Data` buffer.
    ///
    /// Similar to `Stream::poll_next`, this yields `Some(Data)` until
    /// the body ends, when it yields `None`.
    fn poll_data(
        self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>>;

    /// Poll for an optional **single** `HeaderMap` of trailers.
    ///
    /// This should **only** be called after `poll_data` has ended.
    ///
    /// Note: Trailers aren't currently used for HTTP/1, only for HTTP/2.
    fn poll_trailers(
        self: Pin<&mut Self>,
        _cx: &mut task::Context<'_>,
    ) -> Poll<Result<Option<HeaderMap>, Self::Error>> {
        Poll::Ready(Ok(None))
    }

    /// A hint that the `Body` is complete, and doesn't need to be polled more.
    ///
    /// This can be useful to determine if the there is any body or trailers
    /// without having to poll. An empty `Body` could return `true` and hyper
    /// would be able to know that only the headers need to be sent. Or, it can
    /// also be checked after each `poll_data` call, to allow hyper to try to
    /// end the underlying stream with the last chunk, instead of needing to
    /// send an extra `DATA` frame just to mark the stream as finished.
    ///
    /// As a hint, it is used to try to optimize, and thus is OK for a default
    /// implementation to return `false`.
    fn is_end_stream(&self) -> bool {
        false
    }

    /// Returns a `SizeHint` providing an upper and lower bound on the possible size.
    ///
    /// If there is an exact size of bytes known, this would allow hyper to
    /// send a `Content-Length` header automatically, not needing to fall back to
    /// `TransferEncoding: chunked`.
    ///
    /// This does not need to be kept updated after polls, it will only be
    /// called once to create the headers.
    fn size_hint(&self) -> SizeHint {
        SizeHint::default()
    }
}

impl<T> Payload for T
where
    T: HttpBody + Send + 'static,
    T::Data: Send,
    T::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    type Data = T::Data;
    type Error = T::Error;

    fn poll_data(
        self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        HttpBody::poll_data(self, cx)
    }

    fn poll_trailers(
        self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<Option<HeaderMap>, Self::Error>> {
        HttpBody::poll_trailers(self, cx)
    }

    fn is_end_stream(&self) -> bool {
        HttpBody::is_end_stream(self)
    }

    fn size_hint(&self) -> SizeHint {
        HttpBody::size_hint(self)
    }
}

impl<T> sealed::Sealed for T
where
    T: HttpBody + Send + 'static,
    T::Data: Send,
    T::Error: Into<Box<dyn StdError + Send + Sync>>,
{
}

mod sealed {
    pub trait Sealed {}
}

/*
impl<E: Payload> Payload for Box<E> {
    type Data = E::Data;
    type Error = E::Error;

    fn poll_data(&mut self) -> Poll<Option<Self::Data>, Self::Error> {
        (**self).poll_data()
    }

    fn poll_trailers(&mut self) -> Poll<Option<HeaderMap>, Self::Error> {
        (**self).poll_trailers()
    }

    fn is_end_stream(&self) -> bool {
        (**self).is_end_stream()
    }

    fn content_length(&self) -> Option<u64> {
        (**self).content_length()
    }

    #[doc(hidden)]
    fn __hyper_full_data(&mut self, arg: FullDataArg) -> FullDataRet<Self::Data> {
        (**self).__hyper_full_data(arg)
    }
}
*/
