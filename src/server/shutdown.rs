use std::error::Error as StdError;

use pin_project::{pin_project, project};
use tokio::io::{AsyncRead, AsyncWrite};

use super::conn::{SpawnAll, UpgradeableConnection, Watcher};
use super::Accept;
use crate::body::{Body, Payload};
use crate::common::drain::{self, Draining, Signal, Watch, Watching};
use crate::common::exec::{H2Exec, NewSvcExec};
use crate::common::{task, Future, Pin, Poll, Unpin};
use crate::service::{HttpService, MakeServiceRef};

#[allow(missing_debug_implementations)]
#[pin_project]
pub struct Graceful<I, S, F, E> {
    #[pin]
    state: State<I, S, F, E>,
}

#[pin_project]
pub(super) enum State<I, S, F, E> {
    Running {
        drain: Option<(Signal, Watch)>,
        #[pin]
        spawn_all: SpawnAll<I, S, E>,
        #[pin]
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
    I: Accept<Conn = IO, Error = IE>,
    IE: Into<Box<dyn StdError + Send + Sync>>,
    IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    S: MakeServiceRef<IO, Body, ResBody = B>,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
    B: Payload,
    F: Future<Output = ()>,
    E: H2Exec<<S::Service as HttpService<Body>>::Future, B>,
    E: NewSvcExec<IO, S::Future, S::Service, E, GracefulWatcher>,
{
    type Output = crate::Result<()>;

    #[project]
    fn poll(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        let mut me = self.project();
        loop {
            let next = {
                #[project]
                match me.state.as_mut().project() {
                    State::Running {
                        drain,
                        spawn_all,
                        signal,
                    } => match signal.poll(cx) {
                        Poll::Ready(()) => {
                            debug!("signal received, starting graceful shutdown");
                            let sig = drain.take().expect("drain channel").0;
                            State::Draining(sig.drain())
                        }
                        Poll::Pending => {
                            let watch = drain.as_ref().expect("drain channel").1.clone();
                            return spawn_all.poll_watch(cx, &GracefulWatcher(watch));
                        }
                    },
                    State::Draining(ref mut draining) => {
                        return Pin::new(draining).poll(cx).map(Ok);
                    }
                }
            };
            me.state.set(next);
        }
    }
}

#[allow(missing_debug_implementations)]
#[derive(Clone)]
pub struct GracefulWatcher(Watch);

impl<I, S, E> Watcher<I, S, E> for GracefulWatcher
where
    I: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    S: HttpService<Body>,
    E: H2Exec<S::Future, S::ResBody>,
{
    type Future =
        Watching<UpgradeableConnection<I, S, E>, fn(Pin<&mut UpgradeableConnection<I, S, E>>)>;

    fn watch(&self, conn: UpgradeableConnection<I, S, E>) -> Self::Future {
        self.0.clone().watch(conn, on_drain)
    }
}

fn on_drain<I, S, E>(conn: Pin<&mut UpgradeableConnection<I, S, E>>)
where
    S: HttpService<Body>,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
    I: AsyncRead + AsyncWrite + Unpin,
    S::ResBody: Payload + 'static,
    E: H2Exec<S::Future, S::ResBody>,
{
    conn.graceful_shutdown()
}
