#[cfg(any(feature = "http2", test))]
mod compat;
mod rewind;

#[cfg(any(feature = "http2", test))]
pub(crate) use self::compat::{compat, Compat};
pub(crate) use self::rewind::Rewind;
