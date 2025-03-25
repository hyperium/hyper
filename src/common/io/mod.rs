#[cfg(any(http2_client, http2_server))]
mod compat;
mod rewind;

#[cfg(any(http2_client, http2_server))]
pub(crate) use self::compat::Compat;
pub(crate) use self::rewind::Rewind;
