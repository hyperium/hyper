//! The Hyper Body, which is a wrapper around the tokio_proto Body.
//! This is used as the Body for a Server Request, Server Response,
//! and also for a Client Request and Client Response.
//! It is based on a tokio_proto::streaming::Body for now, but will be
//! changed to tokio_proto::multiplex::Body int the future for SSL support.

use std::convert::From;

use tokio_proto;
use http::Chunk;
use futures::{Poll, Stream, Sink};
use futures::sync::mpsc;
use futures::StartSend;

pub type TokioBody = tokio_proto::streaming::Body<Chunk, ::Error>;

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

impl From<Vec<u8>> for Body {
    fn from (vec: Vec<u8>) -> Body {
        let (mut tx, rx) = Body::pair();
        tx.start_send(Ok(Chunk::from(vec)));
        tx.poll_complete();
        rx
    }
}

impl From<&'static [u8]> for Body {
    fn from (static_u8: &'static [u8]) -> Body {
        let vec = static_u8.to_vec();
        Into::<Body>::into(vec)
    }
}

impl From<&'static str> for Body {
    fn from (static_str: &'static str) -> Body {
        let vec = static_str.as_bytes().to_vec();
        Into::<Body>::into(vec)
    }
}