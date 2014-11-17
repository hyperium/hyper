//! A Collection of Header implementations for common HTTP Headers.
//!
//! ## Mime
//!
//! Several header fields use MIME values for their contents. Keeping with the
//! strongly-typed theme, the [mime](http://seanmonstar.github.io/mime.rs) crate
//! is used, such as `ContentType(pub Mime)`.

pub use self::accept::Accept;
pub use self::authorization::Authorization;
pub use self::cookie::Cookies;
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
pub use self::set_cookie::SetCookie;

macro_rules! bench_header(
    ($name:ident, $ty:ty, $value:expr) => {
        #[cfg(test)]
        mod $name {
            use test::Bencher;
            use std::fmt::{mod, Show};

            use super::*;

            use header::{Header, HeaderFormat};

            struct HeaderFormatter($ty);

            impl Show for HeaderFormatter {
                fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    self.0.fmt_header(f)
                }
            }

            #[bench]
            fn bench_parse(b: &mut Bencher) {
                let val = $value;
                b.iter(|| {
                    let _: $ty= Header::parse_header(val[]).unwrap();
                });
            }

            #[bench]
            fn bench_format(b: &mut Bencher) {
                let val = HeaderFormatter(Header::parse_header($value[]).unwrap());
                b.iter(|| {
                    format!("{}", val);
                });
            }
        }
    }
)

/// Exposes the Accept header.
pub mod accept;

/// Exposes the Authorization header.
pub mod authorization;

/// Exposes the Cookie header.
pub mod cookie;

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

/// Exposes the Location header.
pub mod location;

/// Exposes the Server header.
pub mod server;

/// Exposes the Set-Cookie header.
pub mod set_cookie;

/// Exposes the TransferEncoding header.
pub mod transfer_encoding;

/// Exposes the Upgrade header.
pub mod upgrade;

/// Exposes the UserAgent header.
pub mod user_agent;

pub mod util;
