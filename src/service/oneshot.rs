// TODO: Eventually to be replaced with tower_util::Oneshot.

use std::marker::Unpin;
use std::mem;

use tower_service::Service;

use crate::common::{task, Future, Pin, Poll};

pub(crate) fn oneshot<S, Req>(svc: S, req: Req) -> Oneshot<S, Req>
where
    S: Service<Req>,
{
    Oneshot {
        state: State::NotReady(svc, req),
    }
}

// A `Future` consuming a `Service` and request, waiting until the `Service`
// is ready, and then calling `Service::call` with the request, and
// waiting for that `Future`.
#[allow(missing_debug_implementations)]
pub struct Oneshot<S: Service<Req>, Req> {
    state: State<S, Req>,
}

enum State<S: Service<Req>, Req> {
    NotReady(S, Req),
    Called(S::Future),
    Tmp,
}

// Unpin is projected to S::Future, but never S.
impl<S, Req> Unpin for Oneshot<S, Req>
where
    S: Service<Req>,
    S::Future: Unpin,
{
}

impl<S, Req> Future for Oneshot<S, Req>
where
    S: Service<Req>,
{
    type Output = Result<S::Response, S::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        // Safety: The service's future is never moved once we get one.
        let mut me = unsafe { Pin::get_unchecked_mut(self) };

        loop {
            match me.state {
                State::NotReady(ref mut svc, _) => {
                    ready!(svc.poll_ready(cx))?;
                    // fallthrough out of the match's borrow
                }
                State::Called(ref mut fut) => {
                    return unsafe { Pin::new_unchecked(fut) }.poll(cx);
                }
                State::Tmp => unreachable!(),
            }

            match mem::replace(&mut me.state, State::Tmp) {
                State::NotReady(mut svc, req) => {
                    me.state = State::Called(svc.call(req));
                }
                _ => unreachable!(),
            }
        }
    }
}
