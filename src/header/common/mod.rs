//! A Collection of Header implementations for common HTTP Headers.
//!
//! ## Mime
//!
//! Several header fields use MIME values for their contents. Keeping with the
//! strongly-typed theme, the [mime](http://seanmonstar.github.io/mime.rs) crate
//! is used, such as `ContentType(pub Mime)`.

pub use self::accept::Accept;
pub use self::connection::Connection;
pub use self::content_length::ContentLength;
pub use self::content_type::ContentType;
pub use self::date::Date;
pub use self::host::Host;
pub use self::location::Location;
pub use self::transfer_encoding::TransferEncoding;
pub use self::upgrade::Upgrade;
pub use self::user_agent::UserAgent;
pub use self::server::Server;

use std::fmt::{mod, Show};
use std::from_str::FromStr;
use std::str::from_utf8;

/// Exposes the Accept header.
pub mod accept;

/// Exposes the Connection header.
pub mod connection;

/// Exposes the ContentLength header.
pub mod content_length;

/// Exposes the ContentType header.
pub mod content_type;

/// Exposes the Date header.
pub mod date;

/// Exposes the Host header.
pub mod host;

/// Exposes the Server header.
pub mod server;

/// Exposes the TransferEncoding header.
pub mod transfer_encoding;

/// Exposes the Upgrade header.
pub mod upgrade;

/// Exposes the UserAgent header.
pub mod user_agent;


/// Exposes the Location header.
pub mod location;

pub mod util;


fn from_comma_delimited<T: FromStr>(raw: &[Vec<u8>]) -> Option<Vec<T>> {
    if raw.len() != 1 {
        return None;
    }
    // we JUST checked that raw.len() == 1, so raw[0] WILL exist.
    match from_utf8(unsafe { raw.as_slice().unsafe_get(0).as_slice() }) {
        Some(s) => {
            Some(s.as_slice()
                 .split([',', ' '].as_slice())
                 .filter_map(from_str)
                 .collect())
        }
        None => None
    }
}

fn fmt_comma_delimited<T: Show>(fmt: &mut fmt::Formatter, parts: &[T]) -> fmt::Result {
    let last = parts.len() - 1;
    for (i, part) in parts.iter().enumerate() {
        try!(part.fmt(fmt));
        if i < last {
            try!(", ".fmt(fmt));
        }
    }
    Ok(())
}
