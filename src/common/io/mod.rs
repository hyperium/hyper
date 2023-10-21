#[cfg(all(any(feature = "client", feature = "server"), feature = "http2"))]
mod compat;
#[cfg(feature = "upgrade")]
mod rewind;

#[cfg(all(any(feature = "client", feature = "server"), feature = "http2"))]
pub(crate) use self::compat::{compat, Compat};
#[cfg(feature = "upgrade")]
pub(crate) use self::rewind::Rewind;
