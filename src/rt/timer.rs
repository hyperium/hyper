//! Provides a timer trait with timer-like functions
//!
//! Example using tokio timer:
//! ```rust
//! use std::{
//!     pin::Pin,
//!     task::{Context, Poll},
//!     time::{Duration, Instant},
//! };
//!
//! use futures_util::Future;
//! use pin_project_lite::pin_project;
//! use hyper::rt::timer::{Timer, Sleep};
//!
//! #[derive(Clone, Debug)]
//! pub struct TokioTimer;
//!
//! impl Timer for TokioTimer {
//!     fn sleep(&self, duration: Duration) -> Pin<Box<dyn Sleep>> {
//!         Box::pin(TokioSleep {
//!             inner: tokio::time::sleep(duration),
//!         })
//!     }
//!
//!     fn sleep_until(&self, deadline: Instant) -> Pin<Box<dyn Sleep>> {
//!         Box::pin(TokioSleep {
//!             inner: tokio::time::sleep_until(deadline.into()),
//!         })
//!     }
//!
//!     fn reset(&self, sleep: &mut Pin<Box<dyn Sleep>>, new_deadline: Instant) {
//!         if sleep.downcast_ref::<TokioSleep>().is_some() {
//!             *sleep = self.sleep_until(new_deadline);
//!         }
//!     }
//! }
//!
//! pin_project! {
//!     pub(crate) struct TokioSleep {
//!         #[pin]
//!         pub(crate) inner: tokio::time::Sleep,
//!     }
//! }
//!
//! impl Future for TokioSleep {
//!     type Output = ();
//!
//!     fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
//!         self.project().inner.poll(cx)
//!     }
//! }
//!
//! impl Sleep for TokioSleep {}
//! ````

use std::{
    any::TypeId,
    future::Future,
    pin::Pin,
    time::{Duration, Instant},
};

/// A timer which provides timer-like functions.
pub trait Timer {
    /// Return a future that resolves in `duration` time.
    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Sleep>>;

    /// Return a future that resolves at `deadline`.
    fn sleep_until(&self, deadline: Instant) -> Pin<Box<dyn Sleep>>;

    /// Reset a future to resolve at `new_deadline` instead.
    fn reset(&self, sleep: &mut Pin<Box<dyn Sleep>>, new_deadline: Instant) {
        *sleep = self.sleep_until(new_deadline);
    }
}

/// A future returned by a `Timer`.
pub trait Sleep: Send + Sync + Future<Output = ()> {
    #[doc(hidden)]
    /// This method is private and should not be implemented by downstream crate
    fn __type_id(&self, _: private::Sealed) -> TypeId
    where
        Self: 'static,
    {
        TypeId::of::<Self>()
    }
}

impl dyn Sleep {
    //! This is a re-implementation of downcast methods from std::any::Any

    /// Check whether the type is the same as `T`
    pub fn is<T>(&self) -> bool
    where
        T: Sleep + 'static,
    {
        self.__type_id(private::Sealed {}) == TypeId::of::<T>()
    }

    /// Downcast the Sleep object to its original type
    pub fn downcast_ref<T>(&self) -> Option<&T>
    where
        T: Sleep + 'static,
    {
        if self.is::<T>() {
            unsafe { Some(&*(self as *const dyn Sleep as *const T)) }
        } else {
            None
        }
    }

    /// Similar to `downcast_ref` but returns a mutable version instead
    pub fn downcast_mut<T>(&mut self) -> Option<&mut T>
    where
        T: Sleep + 'static,
    {
        if self.is::<T>() {
            unsafe { Some(&mut *(self as *mut dyn Sleep as *mut T)) }
        } else {
            None
        }
    }
}

mod private {
    #![allow(missing_debug_implementations)]
    pub struct Sealed {}
}
