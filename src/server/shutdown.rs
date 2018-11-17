use futures::{Async, Future, Stream, Poll};
use tokio_io::{AsyncRead, AsyncWrite};

use body::{Body, Payload};
use common::drain::{self, Draining, Signal, Watch, Watching};
use common::exec::{H2Exec, NewSvcExec};
use service::Service;
use super::acceptor::Acceptor;
use super::conn::{MakeServiceRef, SpawnAll, UpgradeableConnection, Watcher};

#[allow(missing_debug_implementations)]
pub struct Graceful<I, S, F, E, A> {
    state: State<I, S, F, E, A>,
}

enum State<I, S, F, E, A> {
    Running {
        drain: Option<(Signal, Watch)>,
        spawn_all: SpawnAll<I, S, E, A>,
        signal: F,
    },
    Draining(Draining),
}

impl<I, S, F, E, A> Graceful<I, S, F, E, A> {
    pub(super) fn new(spawn_all: SpawnAll<I, S, E, A>, signal: F) -> Self {
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


impl<I, S, B, F, E, A> Future for Graceful<I, S, F, E, A>
where
    I: Stream,
    I::Error: Into<Box<::std::error::Error + Send + Sync>>,
    I::Item: AsyncRead + AsyncWrite + Send + 'static,
    A: Acceptor<I::Item>,
    A::Item: Send + 'static,
    S: MakeServiceRef<I::Item, ReqBody=Body, ResBody=B>,
    S::Service: 'static,
    S::Error: Into<Box<::std::error::Error + Send + Sync>>,
    B: Payload,
    F: Future<Item=()>,
    E: H2Exec<<S::Service as Service>::Future, B>,
    E: NewSvcExec<A::Accept, S::Future, S::Service, E, GracefulWatcher>,
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
                        let watch = drain
                            .as_ref()
                            .expect("drain channel")
                            .1
                            .clone();
                        return spawn_all.poll_watch(&GracefulWatcher(watch));
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

#[allow(missing_debug_implementations)]
#[derive(Clone)]
pub struct GracefulWatcher(Watch);

impl<I, S, E> Watcher<I, S, E> for GracefulWatcher
where
    I: AsyncRead + AsyncWrite + Send + 'static,
    S: Service<ReqBody=Body> + 'static,
    E: H2Exec<S::Future, S::ResBody>,
{
    type Future = Watching<UpgradeableConnection<I, S, E>, fn(&mut UpgradeableConnection<I, S, E>)>;

    fn watch(&self, conn: UpgradeableConnection<I, S, E>) -> Self::Future {
        self
            .0
            .clone()
            .watch(conn, on_drain)
    }
}

fn on_drain<I, S, E>(conn: &mut UpgradeableConnection<I, S, E>)
where
    S: Service<ReqBody=Body>,
    S::Error: Into<Box<::std::error::Error + Send + Sync>>,
    I: AsyncRead + AsyncWrite,
    S::ResBody: Payload + 'static,
    E: H2Exec<S::Future, S::ResBody>,
{
    conn.graceful_shutdown()
}

