use futures::{Async, Future, Poll, Stream};
use futures::stream::futures_unordered::FuturesUnordered;
use h2::Reason;
use h2::server::{Builder, Connection, Handshake, SendResponse};
use tokio_io::{AsyncRead, AsyncWrite};

use ::server::Service;
use super::{PipeToSendStream, SendBuf};

pub struct Server<T, S, B>
where
    S: Service,
    B: Stream<Error=::Error>,
    B::Item: AsRef<[u8]> + 'static,
{
    service: S,
    state: State<T, S::Future, B>,
}

enum State<T, F, B> 
where
    B: Stream<Error=::Error>,
    B::Item: AsRef<[u8]> + 'static,
{
    Handshaking(Handshake<T, SendBuf<B::Item>>),
    Serving(Serving<T, F, B>),
}

struct Serving<T, F, B>
where
    B: Stream<Error=::Error>,
    B::Item: AsRef<[u8]> + 'static,
{
    conn: Connection<T, SendBuf<B::Item>>,
    streams: FuturesUnordered<H2Stream<F, B>>,
}


impl<T, S, B> Server<T, S, B>
where
    T: AsyncRead + AsyncWrite,
    S: Service<Request = ::server::Request, Response = ::server::Response<B>, Error = ::Error>,
    B: Stream<Error=::Error>,
    B::Item: AsRef<[u8]> + 'static,

{
    pub fn new(io: T, service: S) -> Server<T, S, B> {
        let handshake = Builder::new()
            .handshake(io);
        Server {
            state: State::Handshaking(handshake),
            service: service,
        }
    }
}

impl<T, S, B> Future for Server<T, S, B>
where
    T: AsyncRead + AsyncWrite,
    S: Service<Request = ::server::Request, Response = ::server::Response<B>, Error = ::Error>,
    B: Stream<Error=::Error>,
    B::Item: AsRef<[u8]> + 'static,
{
    type Item = ();
    type Error = ::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            let next = match self.state {
                State::Handshaking(ref mut h) => {
                    let conn = try_ready!(h.poll().map_err(::Error::from_h2));
                    let streams = FuturesUnordered::new();
                    State::Serving(Serving {
                        conn: conn,
                        streams: streams,
                    })
                },
                State::Serving(ref mut srv) => {
                    return srv.poll_server(&mut self.service);
                }
            };
            self.state = next;
        }
    }
}

impl<T, F, B> Serving<T, F, B>
where
    T: AsyncRead + AsyncWrite,
    F: Future<Item=::server::Response<B>, Error=::Error>,
    B: Stream<Error=::Error>,
    B::Item: AsRef<[u8]> + 'static,
{
    fn poll_server<S>(&mut self, service: &mut S) -> Poll<(), ::Error>
    where
        S: Service<Request = ::server::Request, Response = ::server::Response<B>, Error = ::Error, Future=F>,
    {
        loop {
            // always be acceptin'
            'accept: loop {
                match self.conn.poll().map_err(::Error::from_h2)? {
                    Async::Ready(Some((req, respond))) => {
                        trace!("incoming request");
                        let req = req.map(::Body::h2).into();
                        let fut = H2Stream::new(service.call(req), respond);
                        self.streams.push(fut);
                    },
                    Async::Ready(None) => {
                        // no more incoming streams...
                        // do we close now? or try to let streams finish?
                        trace!("incoming connection complete; current streams = {}", self.streams.len());
                        return Ok(Async::Ready(()));
                    }
                    Async::NotReady => {
                        break 'accept;
                    },
                }
            }

            match self.streams.poll() {
                Ok(Async::Ready(Some(()))) => {},
                Ok(Async::Ready(None)) => {
                    return Ok(Async::NotReady);
                },
                Ok(Async::NotReady) => return Ok(Async::NotReady),
                Err(e) => {
                    debug!("stream error: {}", e);
                },
            }
        }
    }
}

struct H2Stream<F, B>
where
    B: Stream<Error=::Error>,
    B::Item: AsRef<[u8]> + 'static,
{
    reply: SendResponse<SendBuf<B::Item>>,
    state: H2StreamState<F, B>,
}

enum H2StreamState<F, B>
where
    B: Stream,
    B::Item: AsRef<[u8]> + 'static,
{
    Service(F),
    Body(PipeToSendStream<B>),
}

impl<F, B> H2Stream<F, B>
where
    F: Future<Item=::server::Response<B>, Error=::Error>,
    B: Stream<Error=::Error>,
    B::Item: AsRef<[u8]> + 'static,
{
    fn new(fut: F, respond: SendResponse<SendBuf<B::Item>>) -> H2Stream<F, B> {
        H2Stream {
            reply: respond,
            state: H2StreamState::Service(fut),
        }
    }

    fn poll2(&mut self) -> Poll<(), ::Error> {
        loop {
            let next = match self.state {
                H2StreamState::Service(ref mut h) => {
                    let res = try_ready!(h.poll()).into_http();
                    let (head, body) = res.into_parts();
                    let mut res = ::http::Response::from_parts(head, ());
                    super::strip_connection_headers(res.headers_mut());
                    macro_rules! reply {
                        ($eos:expr) => ({
                            match self.reply.send_response(res, $eos) {
                                Ok(tx) => tx,
                                Err(e) => {
                                    trace!("send response error: {}", e);
                                    self.reply.send_reset(Reason::INTERNAL_ERROR);
                                    return Err(::Error::from_h2(e));
                                }
                            }
                        })
                    }
                    if let Some(body) = body {
                        let body_tx = reply!(false);
                        H2StreamState::Body(PipeToSendStream::new(body, body_tx))
                    } else {
                        reply!(true);
                        return Ok(Async::Ready(()));
                    }
                },
                H2StreamState::Body(ref mut pipe) => {
                    return pipe.poll();
                }
            };
            self.state = next;
        }
    }
}

impl<F, B> Future for H2Stream<F, B>
where
    F: Future<Item=::server::Response<B>, Error=::Error>,
    B: Stream<Error=::Error>,
    B::Item: AsRef<[u8]> + 'static,
{
    type Item = ();
    type Error = ::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.poll2()
    }
}

