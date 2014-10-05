//! A Collection of Header implementations for common HTTP Headers.
//!
//! ## Mime
//!
//! Several header fields use MIME values for their contents. Keeping with the
//! strongly-typed theme, the [mime](http://seanmonstar.github.io/mime.rs) crate
//! is used, such as `ContentType(pub Mime)`.

pub use self::host::Host;
pub use self::content_length::ContentLength;
pub use self::content_type::ContentType;
pub use self::accept::Accept;
pub use self::connection::Connection;
pub use self::transfer_encoding::TransferEncoding;
pub use self::user_agent::UserAgent;
pub use self::server::Server;
pub use self::date::Date;
pub use self::location::Location;

use std::from_str::FromStr;
use std::str::from_utf8;

/// Exposes the Host header.
pub mod host;

/// Exposes the ContentLength header.
pub mod content_length;

/// Exposes the ContentType header.
pub mod content_type;

/// Exposes the Accept header.
pub mod accept;

/// Exposes the Connection header.
pub mod connection;

/// Exposes the TransferEncoding header.
pub mod transfer_encoding;

/// Exposes the UserAgent header.
pub mod user_agent;

/// Exposes the Server header.
pub mod server;

/// Exposes the Date header.
pub mod date;

/// Exposes the Location header.
pub mod location;

fn from_one_raw_str<T: FromStr>(raw: &[Vec<u8>]) -> Option<T> {
    if raw.len() != 1 {
        return None;
    }
    // we JUST checked that raw.len() == 1, so raw[0] WILL exist.
    match from_utf8(unsafe { raw.as_slice().unsafe_get(0).as_slice() }) {
        Some(s) => FromStr::from_str(s),
        None => None
    }
}

