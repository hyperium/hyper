//! Default runtime
//!
//! By default, hyper includes the [tokio](https://tokio.rs) runtime. To ease
//! using it, several types are re-exported here.
//!
//! The inclusion of a default runtime can be disabled by turning off hyper's
//! `runtime` Cargo feature.

pub use std::future::Future;
pub use futures_core::Stream;

use tokio;

pub use tokio::main;

use self::inner::Spawn;

/// Spawns a future on the default executor.
///
/// # Panics
///
/// This function will panic if the default executor is not set.
///
/// # Note
///
/// The `Spawn` return type is not currently meant for anything other than
/// to reserve adding new trait implementations to it later. It can be
/// ignored for now.
pub fn spawn<F>(f: F) -> Spawn
where
    F: Future<Output = ()> + Send + 'static,
{
    tokio::spawn(f);
    Spawn {
        _inner: (),
    }
}

// Make the `Spawn` type an unnameable, so we can add
// methods or trait impls to it later without a breaking change.
mod inner {
    #[allow(missing_debug_implementations)]
    pub struct Spawn {
        pub(super) _inner: (),
    }
}
