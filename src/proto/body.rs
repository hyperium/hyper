use bytes::Bytes;
use futures::{Poll, Stream};
use futures::sync::mpsc;
use tokio_proto;
use std::borrow::Cow;

use super::Chunk;

pub type TokioBody = tokio_proto::streaming::Body<Chunk, ::Error>;
pub type BodySender = mpsc::Sender<Result<Chunk, ::Error>>;

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
        match cow {
            Cow::Borrowed(b) => Body::from(b),
            Cow::Owned(o) => Body::from(o)
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
        match cow {
            Cow::Borrowed(b) => Body::from(b),
            Cow::Owned(o) => Body::from(o)
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
