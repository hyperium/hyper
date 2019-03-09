use futures::{Async, Stream};
use super::Chunk;

/// A stream that limits data fed to it
///
/// If data come in exceeds limit, they will be _dropped_. No error will be
/// returned. This is mentioned because sometimes one might expect an error
/// when too much data come in.
///
/// # Examples
///
/// ```
/// use hyper::{
///     Body, Client, Request,
///     body::TakeBytesExt,
///     rt::{Future, Stream},
/// };
///
/// let request = Request::get("http://www.rust-lang.org/")
///     .body(Body::empty())
///     .unwrap();
/// let future = Client::new().request(request)
///     .and_then(|response| response.into_body().take_bytes(1024).concat2());
/// // ...
/// ```
#[must_use = "streams do nothing unless polled"]
#[derive(Debug)]
pub struct TakeBytes<S> {
    inner: S,
    limit: usize,
    taken: usize,
}

impl<S> TakeBytes<S> {

    /// Creates new stream with a limit
    fn new(inner: S, limit: usize) -> Self {
        Self {
            inner,
            limit,
            taken: 0,
        }
    }

}

impl<S> Stream for TakeBytes<S> where S: Stream<Item=Chunk> {

    type Item = Chunk;
    type Error = S::Error;

    fn poll(&mut self) -> Result<Async<Option<Self::Item>>, Self::Error> {
        if self.taken >= self.limit {
            return Ok(Async::Ready(None));
        }

        match self.inner.poll() {
            Ok(Async::Ready(Some(mut chunk))) => {
                let chunk_len = chunk.as_ref().len();
                let keep = (self.limit - self.taken).min(chunk_len);
                if keep < chunk_len {
                    let mut bytes = chunk.into_bytes();
                    bytes.truncate(keep);
                    chunk = bytes.into();
                }
                self.taken += keep;
                Ok(Async::Ready(Some(chunk)))
            },
            other => other,
        }
    }

}

/// An extension for [`TakeBytes`](struct.TakeBytes.html)
///
/// Examples are available in that struct's documentation.
pub trait TakeBytesExt: Sized {

    /// Takes at most `limit` bytes from this stream
    fn take_bytes(self, limit: usize) -> TakeBytes<Self>;

}

impl<S> TakeBytesExt for S where S: Stream<Item=Chunk> {

    fn take_bytes(self, limit: usize) -> TakeBytes<Self> {
        TakeBytes::new(self, limit)
    }

}

#[test]
fn test_body_take_bytes() {
    use futures::{Future, Stream};
    use super::Body;

    let body = Body::from("sometimes");
    let chunks = body.take_bytes(4).concat2().wait().unwrap();
    assert_eq!(b"some", chunks.into_bytes().as_ref());

    let body = Body::from("sometimes");
    let chunks = body.take_bytes(100).concat2().wait().unwrap();
    assert_eq!(b"sometimes", chunks.into_bytes().as_ref());
}
