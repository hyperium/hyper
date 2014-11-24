//! The HTTP request method
use std::fmt;
use std::str::FromStr;

use self::Method::{Options, Get, Post, Put, Delete, Head, Trace, Connect, Patch,
                   Extension};

/// The Request Method (VERB)
///
/// Currently includes 8 variants representing the 8 methods defined in
/// [RFC 7230](https://tools.ietf.org/html/rfc7231#section-4.1), plus PATCH,
/// and an Extension variant for all extensions.
///
/// It may make sense to grow this to include all variants currently
/// registered with IANA, if they are at all common to use.
#[deriving(Clone, PartialEq, Eq, Hash)]
pub enum Method {
    /// OPTIONS
    Options,
    /// GET
    Get,
    /// POST
    Post,
    /// PUT
    Put,
    /// DELETE
    Delete,
    /// HEAD
    Head,
    /// TRACE
    Trace,
    /// CONNECT
    Connect,
    /// PATCH
    Patch,
    /// Method extentions. An example would be `let m = Extension("FOO".to_string())`.
    Extension(String)
}

impl Method {
    /// Whether a method is considered "safe", meaning the request is
    /// essentially read-only.
    ///
    /// See [the spec](https://tools.ietf.org/html/rfc7231#section-4.2.1)
    /// for more words.
    pub fn safe(&self) -> bool {
        match *self {
            Get | Head | Options | Trace => true,
            _ => false
        }
    }

    /// Whether a method is considered "idempotent", meaning the request has
    /// the same result is executed multiple times.
    ///
    /// See [the spec](https://tools.ietf.org/html/rfc7231#section-4.2.2) for
    /// more words.
    pub fn idempotent(&self) -> bool {
        if self.safe() {
            true
        } else {
            match *self {
                Put | Delete => true,
                _ => false
            }
        }
    }
}

impl FromStr for Method {
    fn from_str(s: &str) -> Option<Method> {
        Some(match s {
            "OPTIONS" => Options,
            "GET" => Get,
            "POST" => Post,
            "PUT" => Put,
            "DELETE" => Delete,
            "HEAD" => Head,
            "TRACE" => Trace,
            "CONNECT" => Connect,
            "PATCH" => Patch,
            _ => Extension(s.to_string())
        })
    }
}

impl fmt::Show for Method {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Options => "OPTIONS",
            Get => "GET",
            Post => "POST",
            Put => "PUT",
            Delete => "DELETE",
            Head => "HEAD",
            Trace => "TRACE",
            Connect => "CONNECT",
            Patch => "PATCH",
            Extension(ref s) => s.as_slice()
        }.fmt(fmt)
    }
}
