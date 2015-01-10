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
