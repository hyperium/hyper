use futures::{Async, Future, Stream, Poll};
use tokio_io::{AsyncRead, AsyncWrite};

use body::{Body, Payload};
use common::drain::{self, Draining, Signal, Watch};
use service::{Service, NewService};
use super::SpawnAll;

#[allow(missing_debug_implementations)]
pub struct Graceful<I, S, F> {
    state: State<I, S, F>,
}

enum State<I, S, F> {
    Running {
        drain: Option<(Signal, Watch)>,
        spawn_all: SpawnAll<I, S>,
        signal: F,
    },
    Draining(Draining),
}

impl<I, S, F> Graceful<I, S, F> {
    pub(super) fn new(spawn_all: SpawnAll<I, S>, signal: F) -> Self {
        let drain = Some(drain::channel());
        Graceful {
            state: State::Running {
                drain,
                spawn_all,
                signal,
            },
        }
    }
}


impl<I, S, B, F> Future for Graceful<I, S, F>
where
    I: Stream,
    I::Error: Into<Box<::std::error::Error + Send + Sync>>,
    I::Item: AsyncRead + AsyncWrite + Send + 'static,
    S: NewService<ReqBody=Body, ResBody=B> + Send + 'static,
    S::Error: Into<Box<::std::error::Error + Send + Sync>>,
    S::Service: Send,
    S::Future: Send + 'static,
    <S::Service as Service>::Future: Send + 'static,
    B: Payload,
    F: Future<Item=()>,
{
    type Item = ();
    type Error = ::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            let next = match self.state {
                State::Running {
                    ref mut drain,
                    ref mut spawn_all,
                    ref mut signal,
                } => match signal.poll() {
                    Ok(Async::Ready(())) | Err(_) => {
                        debug!("signal received, starting graceful shutdown");
                        let sig = drain
                            .take()
                            .expect("drain channel")
                            .0;
                        State::Draining(sig.drain())
                    },
                    Ok(Async::NotReady) => {
                        let watch = &drain
                            .as_ref()
                            .expect("drain channel")
                            .1;
                        return spawn_all.poll_with(|| {
                            let watch = watch.clone();
                            move |conn| {
                                watch.watch(conn, |conn| {
                                    // on_drain, start conn graceful shutdown
                                    conn.graceful_shutdown()
                                })
                            }
                        });
                    },
                },
                State::Draining(ref mut draining) => {
                    return draining.poll()
                        .map_err(|()| unreachable!("drain mpsc rx never errors"));
                }
            };
            self.state = next;
        }
    }
}
