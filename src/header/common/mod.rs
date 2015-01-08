//! A Collection of Header implementations for common HTTP Headers.
//!
//! Several header fields use MIME values for their contents. Keeping with the
//! strongly-typed theme, the [mime](//seanmonstar.github.io/mime.rs) crate
//! is used, such as `ContentType(pub Mime)`.
//!
//! Hyper aims to provide all common headers. This list shows which headers
//! are currently supported, grouped by their usage.
//!
//! # Common HTTP/1.1 headers
//! ## Core header fields
//! These header fields are defined by [RFC7231]
//! (//tools.ietf.org/html/rfc7231#section-5.1) and they do not fit into one
//! of the other categories.
//!
//! * ✔ `Connection`
//! * ✔ `Content-Length`
//! * ✘ `Trailer`
//! * ✔ `Transfer-Encoding`
//! * ✔ `Upgrade`
//! * ✘ `Via`
//!
//! ## Request Header Fields
//! ### Controls
//! * ✔ `Cache-Control`
//! * ✘ `Expect`
//! * ✔ `Host`
//! * ✘ `Max-Forwards`
//! * ✘ `Pragma`
//! * ✘ `Range`
//! * ✘ `TE`
//!
//! #### Conditionals
//! The HTTP conditional request header fields allow a client to place a
//! precondition on the state of the target resource, so that
//! the action corresponding to the method semantics will not be applied
//! if the precondition evaluates to false.
//!
//! * ✘ `If-Match`
//! * ✘ `If-None-Match`
//! * ✔ `If-Modified-Since`
//! * ✘ `If-Unmodified-Since`
//! * ✘ `If-Range`
//!
//! ### Content Negotiation
//! * ✔ `Accept`
//! * ✘ `Accept-Charset`
//! * ✔ `Accept-Encoding`
//! * ✘ `Accept-Language`
//!
//! ### Authentication Credentials
//! The two header fields are used for carrying authentication credentials.
//!
//! * ✔ `Authorization`
//! * ✘ `Proxy-Authorization`
//!
//! ### Request Context
//! Request context fields provide additional information about the request
//! context, including information about the user, user agent, and resource
//! behind the request.
//!
//! * ✘ `From`
//! * ✘ `Referer`
//! * ✔ `User-Agent`
//!
//! ## Response Header Fields
//! ### Control Data
//! * ✘ `Age`
//! * ✔ `Cache-Control`
//! * ✔ `Expires`
//! * ✔ `Date`
//! * ✔ `Location`
//! * ✘ `Retry-After`
//! * ✔ `Vary`
//! * ✘ `Warning`
//!
//! ### Validator Header Fields
//! * ✔ `ETag`
//! * ✔ `Last-Modified`
//!
//! ### Authentication Challenges
//! * ✘ `WWW-Authenticate`
//! * ✘ `Proxy-Authenticate`
//!
//! ### Response Context
//! * ✘ `Accept-Ranges`
//! * ✔ `Allow`
//! * ✔ `Server`
//!
//! # Cross-Origin Resource Sharing
//! [Cross-Origin Resource Sharing](//www.w3.org/TR/cors/) (CORS)
//! defines a mechanism to enable client-side cross-origin requests.
//!
//! ## Request Header Fields
//! * ✘ `Origin`
//! * ✔ [`Access-Control-Request-Method`](struct.AccessControlRequestMethod.html)
//! * ✔ [`Access-Control-Request-Headers`](struct.AccessControlRequestHeaders.html)
//!
//! ## Response Header Fields
//! * ✔ [`Access-Control-Allow-Origin`](enum.AccessControlAllowOrigin.html)
//! * ✘ `Access-Control-Allow-Credentials`
//! * ✘ `Access-Control-Expose-Headers`
//! * ✔ [`Access-Control-Max-Age`](struct.AccessControlMaxAge.html)
//! * ✔ [`Access-Control-Allow-Methods`](struct.AccessControlAllowMethods.html)
//! * ✔ [`Access-Control-Allow-Headers`](struct.AccessControlAllowHeaders.html)
//!
//! # Cookies
//! Cookies are defined in [RFC6265]
//! (//tools.ietf.org/html/rfc6265).
//!
//! * ✔ [`Cookie`](struct.Cookies.html)
//! * ✔ [`Set-Cookie`](struct.SetCookie.html)

pub use self::accept::Accept;
pub use self::accept_encoding::AcceptEncoding;
pub use self::access_control::AccessControlAllowHeaders;
pub use self::access_control::AccessControlAllowMethods;
pub use self::access_control::AccessControlAllowOrigin;
pub use self::access_control::AccessControlMaxAge;
pub use self::access_control::AccessControlRequestHeaders;
pub use self::access_control::AccessControlRequestMethod;
pub use self::allow::Allow;
pub use self::authorization::Authorization;
pub use self::cache_control::CacheControl;
pub use self::cookie::Cookies;
pub use self::connection::Connection;
pub use self::content_length::ContentLength;
pub use self::content_type::ContentType;
pub use self::date::Date;
pub use self::etag::Etag;
pub use self::expires::Expires;
pub use self::host::Host;
pub use self::last_modified::LastModified;
pub use self::if_modified_since::IfModifiedSince;
pub use self::location::Location;
pub use self::transfer_encoding::TransferEncoding;
pub use self::upgrade::Upgrade;
pub use self::user_agent::UserAgent;
pub use self::vary::Vary;
pub use self::server::Server;
pub use self::set_cookie::SetCookie;

macro_rules! bench_header(
    ($name:ident, $ty:ty, $value:expr) => {
        #[cfg(test)]
        mod $name {
            use test::Bencher;
            use super::*;

            use header::{Header, HeaderFormatter};

            #[bench]
            fn bench_parse(b: &mut Bencher) {
                let val = $value;
                b.iter(|| {
                    let _: $ty = Header::parse_header(val[]).unwrap();
                });
            }

            #[bench]
            fn bench_format(b: &mut Bencher) {
                let val: $ty = Header::parse_header($value[]).unwrap();
                let fmt = HeaderFormatter(&val);
                b.iter(|| {
                    format!("{}", fmt);
                });
            }
        }
    }
);

macro_rules! deref(
    ($from:ty -> $to:ty) => {
        impl ::std::ops::Deref for $from {
            type Target = $to;

            fn deref<'a>(&'a self) -> &'a $to {
                &self.0
            }
        }

        impl ::std::ops::DerefMut for $from {
            fn deref_mut<'a>(&'a mut self) -> &'a mut $to {
                &mut self.0
            }
        }
    }
);

mod accept;
mod accept_encoding;
mod access_control;
mod allow;
mod authorization;
mod cache_control;
mod cookie;
mod connection;
mod content_length;
mod content_type;
mod date;
mod etag;
mod expires;
mod host;
mod last_modified;
mod if_modified_since;
mod location;
mod server;
mod set_cookie;
mod transfer_encoding;
mod upgrade;
mod user_agent;
mod vary;
