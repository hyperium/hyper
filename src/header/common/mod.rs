//! A Collection of Header implementations for common HTTP Headers.
//!
//! ## Mime
//!
//! Several header fields use MIME values for their contents. Keeping with the
//! strongly-typed theme, the [mime](http://seanmonstar.github.io/mime.rs) crate
//! is used, such as `ContentType(pub Mime)`.

pub use self::access_control::*;
pub use self::accept::Accept;
pub use self::accept_encoding::AcceptEncoding;
pub use self::allow::Allow;
pub use self::authorization::{Authorization, Scheme, Basic};
pub use self::cache_control::{CacheControl, CacheDirective};
pub use self::connection::{Connection, ConnectionOption};
pub use self::content_length::ContentLength;
pub use self::content_type::ContentType;
pub use self::cookie::Cookies;
pub use self::date::Date;
pub use self::etag::Etag;
pub use self::expires::Expires;
pub use self::host::Host;
pub use self::if_modified_since::IfModifiedSince;
pub use self::last_modified::LastModified;
pub use self::location::Location;
pub use self::referer::Referer;
pub use self::server::Server;
pub use self::set_cookie::SetCookie;
pub use self::transfer_encoding::TransferEncoding;
pub use self::upgrade::{Upgrade, Protocol};
pub use self::user_agent::UserAgent;
pub use self::vary::Vary;

#[macro_export]
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
                    let _: $ty = Header::parse_header(&val[]).unwrap();
                });
            }

            #[bench]
            fn bench_format(b: &mut Bencher) {
                let val: $ty = Header::parse_header(&$value[]).unwrap();
                let fmt = HeaderFormatter(&val);
                b.iter(|| {
                    format!("{}", fmt);
                });
            }
        }
    }
);

#[macro_export]
macro_rules! deref(
    ($from:ty => $to:ty) => {
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

#[macro_export]
macro_rules! impl_list_header(
    ($from:ident, $name:expr, $item:ty) => {
        deref!($from => $item);

        impl header::Header for $from {
            fn header_name() -> &'static str {
                $name
            }

            fn parse_header(raw: &[Vec<u8>]) -> Option<$from> {
                $crate::header::parsing::from_comma_delimited(raw).map($from)
            }
        }

        impl header::HeaderFormat for $from {
            fn fmt_header(&self, fmt: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                $crate::header::parsing::fmt_comma_delimited(fmt, &self[])
            }
        }

        impl ::std::fmt::String for $from {
            fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                use header::HeaderFormat;
                self.fmt_header(f)
            }
        }
    }
);

#[macro_export]
macro_rules! impl_header(
    ($from:ident, $name:expr, $item:ty) => {
        deref!($from => $item);

        impl header::Header for $from {
            fn header_name() -> &'static str {
                $name
            }

            fn parse_header(raw: &[Vec<u8>]) -> Option<$from> {
                $crate::header::parsing::from_one_raw_str(raw).map($from)
            }
        }

        impl header::HeaderFormat for $from {
            fn fmt_header(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                ::std::fmt::String::fmt(&**self, f)
            }
        }

        impl ::std::fmt::String for $from {
            fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                use header::HeaderFormat;
                self.fmt_header(f)
            }
        }
    }
);

mod access_control;
mod accept;
mod accept_encoding;
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
mod referer;
mod server;
mod set_cookie;
mod transfer_encoding;
mod upgrade;
mod user_agent;
mod vary;

