use std::error::Error as StdError;

use futures_core::Stream;
use tokio_io::{AsyncRead, AsyncWrite};

use crate::body::{Body, Payload};
use crate::common::drain::{self, Draining, Signal, Watch, Watching};
use crate::common::exec::{H2Exec, NewSvcExec};
use crate::common::{Future, Pin, Poll, Unpin, task};
use crate::service::{MakeServiceRef, Service};
use super::conn::{SpawnAll, UpgradeableConnection, Watcher};

#[allow(missing_debug_implementations)]
pub struct Graceful<I, S, F, E> {
    state: State<I, S, F, E>,
}

enum State<I, S, F, E> {
    Running {
        drain: Option<(Signal, Watch)>,
        spawn_all: SpawnAll<I, S, E>,
        signal: F,
    },
    Draining(Draining),
}

impl<I, S, F, E> Graceful<I, S, F, E> {
    pub(super) fn new(spawn_all: SpawnAll<I, S, E>, signal: F) -> Self {
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


impl<I, IO, IE, S, B, F, E> Future for Graceful<I, S, F, E>
where
    I: Stream<Item=Result<IO, IE>>,
    IE: Into<Box<dyn StdError + Send + Sync>>,
    IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    S: MakeServiceRef<IO, Body, ResBody=B>,
    S::Service: 'static,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
    B: Payload,
    B::Data: Unpin,
    F: Future<Output=()>,
    E: H2Exec<<S::Service as Service<Body>>::Future, B>,
    E: NewSvcExec<IO, S::Future, S::Service, E, GracefulWatcher>,
{
    type Output = crate::Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        // Safety: the futures are NEVER moved, self.state is overwritten instead.
        let me = unsafe { self.get_unchecked_mut() };
        loop {
            let next = match me.state {
                State::Running {
                    ref mut drain,
                    ref mut spawn_all,
                    ref mut signal,
                } => match unsafe { Pin::new_unchecked(signal) }.poll(cx) {
                    Poll::Ready(()) => {
                        debug!("signal received, starting graceful shutdown");
                        let sig = drain
                            .take()
                            .expect("drain channel")
                            .0;
                        State::Draining(sig.drain())
                    },
                    Poll::Pending => {
                        let watch = drain
                            .as_ref()
                            .expect("drain channel")
                            .1
                            .clone();
                        return unsafe { Pin::new_unchecked(spawn_all) }.poll_watch(cx, &GracefulWatcher(watch));
                    },
                },
                State::Draining(ref mut draining) => {
                    return Pin::new(draining).poll(cx).map(Ok);
                }
            };
            // It's important to just assign, not mem::replace or anything.
            me.state = next;
        }
    }
}

#[allow(missing_debug_implementations)]
#[derive(Clone)]
pub struct GracefulWatcher(Watch);

impl<I, S, E> Watcher<I, S, E> for GracefulWatcher
where
    I: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    S: Service<Body> + 'static,
    <S::ResBody as Payload>::Data: Unpin,
    E: H2Exec<S::Future, S::ResBody>,
{
    type Future = Watching<UpgradeableConnection<I, S, E>, fn(Pin<&mut UpgradeableConnection<I, S, E>>)>;

    fn watch(&self, conn: UpgradeableConnection<I, S, E>) -> Self::Future {
        self
            .0
            .clone()
            .watch(conn, on_drain)
    }
}

fn on_drain<I, S, E>(conn: Pin<&mut UpgradeableConnection<I, S, E>>)
where
    S: Service<Body>,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
    I: AsyncRead + AsyncWrite + Unpin,
    S::ResBody: Payload + 'static,
    <S::ResBody as Payload>::Data: Unpin,
    E: H2Exec<S::Future, S::ResBody>,
{
    conn.graceful_shutdown()
}

