//! Streaming bodies for Requests and Responses
//!
//! For both [Clients](crate::client) and [Servers](crate::server), requests and
//! responses use streaming bodies, instead of complete buffering. This
//! allows applications to not use memory they don't need, and allows exerting
//! back-pressure on connections by only reading when asked.
//!
//! There are two pieces to this in hyper:
//!
//! - **The [`HttpBody`](HttpBody) trait** describes all possible bodies.
//!   hyper allows any body type that implements `HttpBody`, allowing
//!   applications to have fine-grained control over their streaming.
//! - **The [`Recv`](Recv) concrete type**, which is an implementation of
//!   `HttpBody`, and returned by hyper as a "receive stream" (so, for server
//!   requests and client responses). It is also a decent default implementation
//!   if you don't have very custom needs of your send streams.

pub use bytes::{Buf, Bytes};
pub use http_body::Body as HttpBody;
pub use http_body::SizeHint;

pub use self::aggregate::aggregate;
pub use self::body::{Recv, Sender};
pub(crate) use self::length::DecodedLength;
pub use self::to_bytes::to_bytes;

mod aggregate;
mod body;
mod length;
mod to_bytes;

fn _assert_send_sync() {
    fn _assert_send<T: Send>() {}
    fn _assert_sync<T: Sync>() {}

    _assert_send::<Recv>();
    _assert_sync::<Recv>();
}
