//! Lower-level client connection API.
//!
//! The types in this module are to provide a lower-level API based around a
//! single connection. Connecting to a host, pooling connections, and the like
//! are not handled at this level. This module provides the building blocks to
//! customize those things externally.
//!
//! If you are looking for a convenient HTTP client, then you may wish to
//! consider [reqwest](https://github.com/seanmonstar/reqwest) for a high level
//! client or [`hyper-util`'s client](https://docs.rs/hyper-util/latest/hyper_util/client/index.html)
//! if you want to keep it more low level / basic.
//!
//! ## Example
//!
//! See the [client guide](https://hyper.rs/guides/1/client/basic/).
//!
//! ## HTTP/1 vs HTTP/2
//!
//! This module is split by HTTP version: [`http1`] and [`http2`]. After you
//! obtain an IO transport (for example from a TCP or TLS connector), you choose
//! which handshake to run.
//!
//! When using TLS, **ALPN** (Application-Layer Protocol Negotiation) decides
//! which HTTP version **must** be used on that connection:
//!
//! - If the peer negotiated `h2`, call [`http2::handshake`].
//! - If the peer negotiated `http/1.1`, or ALPN was not used, call
//!   [`http1::handshake`].
//!
//! That negotiated choice is final for the life of the connection. Speaking a
//! different HTTP version on the same IO is incorrect.
//!
//! The [`Version`](crate::Version) on a [`Request`](crate::Request) does **not**
//! select HTTP/1 vs HTTP/2 at this layer. Higher-level clients built on these
//! APIs (such as [`hyper-util`'s legacy
//! `Client`](https://docs.rs/hyper-util/latest/hyper_util/client/legacy/struct.Client.html))
//! similarly treat connector / ALPN configuration as authoritative over the
//! request's version field: if the connector reports that `h2` was negotiated,
//! the connection is used as HTTP/2 even when individual requests say
//! `Version::HTTP_11`.
//!
//! To force HTTP/1 only, configure the TLS connector so it does not offer or
//! accept the `h2` ALPN protocol (and do not call [`http2::handshake`]). There
//! is no separate `http1_only` switch in this module; the handshake you run
//! *is* the version selection.

#[cfg(feature = "http1")]
pub mod http1;
#[cfg(feature = "http2")]
pub mod http2;

pub use super::dispatch::TrySendError;
