use std::mem;

use futures::{Future, IntoFuture, Poll};

pub(crate) trait Started: Future {
    fn started(&self) -> bool;
}

pub(crate) fn lazy<F, R>(func: F) -> Lazy<F, R>
where
    F: FnOnce() -> R,
    R: IntoFuture,
{
    Lazy {
        inner: Inner::Init(func),
    }
}

// FIXME: allow() required due to `impl Trait` leaking types to this lint
#[allow(missing_debug_implementations)]
pub(crate) struct Lazy<F, R: IntoFuture> {
    inner: Inner<F, R::Future>
}

enum Inner<F, R> {
    Init(F),
    Fut(R),
    Empty,
}

impl<F, R> Started for Lazy<F, R>
where
    F: FnOnce() -> R,
    R: IntoFuture,
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
    R: IntoFuture,
{
    type Item = R::Item;
    type Error = R::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match self.inner {
            Inner::Fut(ref mut f) => return f.poll(),
            _ => (),
        }

        match mem::replace(&mut self.inner, Inner::Empty) {
            Inner::Init(func) => {
                let mut fut = func().into_future();
                let ret = fut.poll();
                self.inner = Inner::Fut(fut);
                ret
            },
            _ => unreachable!("lazy state wrong"),
        }
    }
}

