use std::borrow::Cow;
use std::mem::replace;

use bytes::{Bytes, BytesMut};
use futures::{Async, Future, Poll, Stream};
use futures::sync::mpsc;
use tokio_proto;

use http::Chunk;
use error::BodyTooLargeError;

pub type TokioBody = tokio_proto::streaming::Body<Chunk, ::Error>;

/// A `Stream` for `Chunk`s used in requests and responses.
#[must_use = "streams do nothing unless polled"]
#[derive(Debug)]
pub struct Body(TokioBody);

impl Body {
    /// Return an empty body stream
    #[inline]
    pub fn empty() -> Body {
        Body(TokioBody::empty())
    }

    /// Return a body stream with an associated sender half
    #[inline]
    pub fn pair() -> (mpsc::Sender<Result<Chunk, ::Error>>, Body) {
        let (tx, rx) = TokioBody::pair();
        let rx = Body(rx);
        (tx, rx)
    }

    /// Buffer body stream with a limit.
    ///
    /// Concatenates chunks from the stream as long
    /// as the resulting buffer is below the size limit
    /// in bytes.
    ///
    /// Fails with `BodyTooLargeError` when the limit is exceeded.
    pub fn buffer(self, limit: usize) -> Buffering {
        buffer(self, limit)
    }
}

impl Default for Body {
    #[inline]
    fn default() -> Body {
        Body::empty()
    }
}

impl Stream for Body {
    type Item = Chunk;
    type Error = ::Error;

    #[inline]
    fn poll(&mut self) -> Poll<Option<Chunk>, ::Error> {
        self.0.poll()
    }
}

impl From<Body> for tokio_proto::streaming::Body<Chunk, ::Error> {
    #[inline]
    fn from(b: Body) -> tokio_proto::streaming::Body<Chunk, ::Error> {
        b.0
    }
}

impl From<tokio_proto::streaming::Body<Chunk, ::Error>> for Body {
    #[inline]
    fn from(tokio_body: tokio_proto::streaming::Body<Chunk, ::Error>) -> Body {
        Body(tokio_body)
    }
}

impl From<mpsc::Receiver<Result<Chunk, ::Error>>> for Body {
    #[inline]
    fn from(src: mpsc::Receiver<Result<Chunk, ::Error>>) -> Body {
        Body(src.into())
    }
}

impl From<Chunk> for Body {
    #[inline]
    fn from (chunk: Chunk) -> Body {
        Body(TokioBody::from(chunk))
    }
}

impl From<Bytes> for Body {
    #[inline]
    fn from (bytes: Bytes) -> Body {
        Body(TokioBody::from(Chunk::from(bytes)))
    }
}

impl From<Vec<u8>> for Body {
    #[inline]
    fn from (vec: Vec<u8>) -> Body {
        Body(TokioBody::from(Chunk::from(vec)))
    }
}

impl From<&'static [u8]> for Body {
    #[inline]
    fn from (slice: &'static [u8]) -> Body {
        Body(TokioBody::from(Chunk::from(slice)))
    }
}

impl From<Cow<'static, [u8]>> for Body {
    #[inline]
    fn from (cow: Cow<'static, [u8]>) -> Body {
        if let Cow::Borrowed(value) = cow {
            Body::from(value)
        } else {
            Body::from(cow.to_owned())
        }
    }
}

impl From<String> for Body {
    #[inline]
    fn from (s: String) -> Body {
        Body(TokioBody::from(Chunk::from(s.into_bytes())))
    }
}

impl From<&'static str> for Body {
    #[inline]
    fn from(slice: &'static str) -> Body {
        Body(TokioBody::from(Chunk::from(slice.as_bytes())))
    }
}

impl From<Cow<'static, str>> for Body {
    #[inline]
    fn from(cow: Cow<'static, str>) -> Body {
        if let Cow::Borrowed(value) = cow {
            Body::from(value)
        } else {
            Body::from(cow.to_owned())
        }
    }
}

impl From<Option<Body>> for Body {
    #[inline]
    fn from (body: Option<Body>) -> Body {
        body.unwrap_or_default()
    }
}

fn _assert_send_sync() {
    fn _assert_send<T: Send>() {}
    fn _assert_sync<T: Sync>() {}

    _assert_send::<Body>();
    _assert_send::<Chunk>();
    _assert_sync::<Chunk>();
}

#[test]
fn test_body_stream_concat() {
    use futures::{Sink, Stream, Future};
    let (tx, body) = Body::pair();

    ::std::thread::spawn(move || {
        let tx = tx.send(Ok("hello ".into())).wait().unwrap();
        tx.send(Ok("world".into())).wait().unwrap();
    });

    let total = body.concat2().wait().unwrap();
    assert_eq!(total.as_ref(), b"hello world");

}

#[derive(Debug)]
/// Future that represents the fully buffered body.
///
/// Can be created with the `Body::buffer` method.
pub struct Buffering {
    limit: usize,
    bytes: BytesMut,
    inner: Body
}

fn buffer(body: Body, limit: usize) -> Buffering {
    Buffering {
        limit: limit,
        bytes: BytesMut::new(),
        inner: body
    }
}

impl Future for Buffering {
    type Item = Result<Bytes, BodyTooLargeError>;
    type Error = ::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match self.inner.poll()? {
            Async::NotReady => Ok(Async::NotReady),
            Async::Ready(Some(chunk)) => {
                if self.bytes.len() + chunk.len() > self.limit {
                    return Ok(Async::Ready(Err(BodyTooLargeError)));
                } else {
                    self.bytes.extend_from_slice(&chunk);
                    self.poll()
                }
            },
            Async::Ready(None) => {
                let bytes = replace(&mut self.bytes, BytesMut::new());
                Ok(Async::Ready(Ok(bytes.freeze())))
            }
        }
    }
}

#[test]
fn test_body_buffer_empty() {
    use futures::Future;
    let body = Body::default();

    let total = body.buffer(0).wait().unwrap().unwrap();
    assert_eq!(total.as_ref(), b"");
}

#[test]
fn test_body_buffer_within_limit() {
    use futures::{Future, Sink};
    let (tx, body) = Body::pair();

    ::std::thread::spawn(move || {
        let tx = tx.send(Ok("hello ".into())).wait().unwrap();
        tx.send(Ok("world".into())).wait().unwrap();
    });

    let total = body.buffer(42).wait().unwrap().unwrap();
    assert_eq!(total.as_ref(), b"hello world");
}

#[test]
fn test_body_buffer_limit_exceeded() {
    use futures::{Future, Sink};
    let (tx, body) = Body::pair();

    ::std::thread::spawn(move || {
        let tx = tx.send(Ok("hello ".into())).wait().unwrap();
        tx.send(Ok("world".into())).wait().unwrap();
    });

    let err = body.buffer(5).wait().unwrap().err();
    assert_eq!(err, Some(BodyTooLargeError));
}
