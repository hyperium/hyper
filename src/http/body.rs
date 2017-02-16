use std::convert::From;

use tokio_proto;
use http::Chunk;
use futures::{Poll, Stream};
use futures::sync::mpsc;

pub type TokioBody = tokio_proto::streaming::Body<Chunk, ::Error>;

/// A `Stream` for `Chunk`s used in requests and responses.
#[derive(Debug)]
pub struct Body(TokioBody);

impl Body {
    /// Return an empty body stream
    pub fn empty() -> Body {
        Body(TokioBody::empty())
    }

    /// Return a body stream with an associated sender half
    pub fn pair() -> (mpsc::Sender<Result<Chunk, ::Error>>, Body) {
        let (tx, rx) = TokioBody::pair();
        let rx = Body(rx);
        (tx, rx)
    }
}

impl Stream for Body {
    type Item = Chunk;
    type Error = ::Error;

    fn poll(&mut self) -> Poll<Option<Chunk>, ::Error> {
        self.0.poll()
    }
}

impl From<Body> for tokio_proto::streaming::Body<Chunk, ::Error> {
    fn from(b: Body) -> tokio_proto::streaming::Body<Chunk, ::Error> {
        b.0
    }
}

impl From<tokio_proto::streaming::Body<Chunk, ::Error>> for Body {
    fn from(tokio_body: tokio_proto::streaming::Body<Chunk, ::Error>) -> Body {
        Body(tokio_body)
    }
}

impl From<mpsc::Receiver<Result<Chunk, ::Error>>> for Body {
    fn from(src: mpsc::Receiver<Result<Chunk, ::Error>>) -> Body {
        Body(src.into())
    }
}

impl From<Chunk> for Body {
    fn from (chunk: Chunk) -> Body {
        Body(TokioBody::from(chunk))
    }
}

impl From<Vec<u8>> for Body {
    fn from (vec: Vec<u8>) -> Body {
        Body(TokioBody::from(Chunk::from(vec)))
    }
}

impl From<&'static [u8]> for Body {
    fn from (slice: &'static [u8]) -> Body {
        Body(TokioBody::from(Chunk::from(slice)))
    }
}

impl From<String> for Body {
    fn from (s: String) -> Body {
        Body(TokioBody::from(Chunk::from(s.into_bytes())))
    }
}

impl From<&'static str> for Body {
    fn from (slice: &'static str) -> Body {
        Body(TokioBody::from(Chunk::from(slice.as_bytes())))
    }
}

fn _assert_send() {
    fn _assert<T: Send>() {}

    _assert::<Body>();
    _assert::<Chunk>();
}
