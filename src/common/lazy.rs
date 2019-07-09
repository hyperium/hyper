use std::mem;

use super::{Future, Pin, Poll, task};

pub(crate) trait Started: Future {
    fn started(&self) -> bool;
}

pub(crate) fn lazy<F, R>(func: F) -> Lazy<F, R>
where
    F: FnOnce() -> R,
    R: Future + Unpin,
{
    Lazy {
        inner: Inner::Init(func),
    }
}

// FIXME: allow() required due to `impl Trait` leaking types to this lint
#[allow(missing_debug_implementations)]
pub(crate) struct Lazy<F, R> {
    inner: Inner<F, R>
}

enum Inner<F, R> {
    Init(F),
    Fut(R),
    Empty,
}

impl<F, R> Started for Lazy<F, R>
where
    F: FnOnce() -> R,
    R: Future + Unpin,
{
    fn started(&self) -> bool {
        match self.inner {
            Inner::Init(_) => false,
            Inner::Fut(_) |
            Inner::Empty => true,
        }
    }
}

impl<F, R> Future for Lazy<F, R>
where
    F: FnOnce() -> R,
    R: Future + Unpin,
{
    type Output = R::Output;

    fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        match self.inner {
            Inner::Fut(ref mut f) => return Pin::new(f).poll(cx),
            _ => (),
        }

        match mem::replace(&mut self.inner, Inner::Empty) {
            Inner::Init(func) => {
                let mut fut = func();
                let ret = Pin::new(&mut fut).poll(cx);
                self.inner = Inner::Fut(fut);
                ret
            },
            _ => unreachable!("lazy state wrong"),
        }
    }
}

// The closure `F` is never pinned
impl<F, R: Unpin> Unpin for Lazy<F, R> {}

