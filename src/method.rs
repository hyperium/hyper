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
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
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
        if s == "" {
            None
        } else {
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
}

impl fmt::Display for Method {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.write_str(match *self {
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
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::str::FromStr;
    use super::Method;
    use super::Method::{Get, Post, Put, Extension};

    #[test]
    fn test_safe() {
        assert_eq!(true, Get.safe());
        assert_eq!(false, Post.safe());
    }

    #[test]
    fn test_idempotent() {
        assert_eq!(true, Get.idempotent());
        assert_eq!(true, Put.idempotent());
        assert_eq!(false, Post.idempotent());
    }

    #[test]
    fn test_from_str() {
        assert_eq!(Some(Get), FromStr::from_str("GET"));
        assert_eq!(Some(Extension("MOVE".to_string())),
                   FromStr::from_str("MOVE"));
    }

    #[test]
    fn test_fmt() {
        assert_eq!("GET".to_string(), format!("{}", Get));
        assert_eq!("MOVE".to_string(),
                   format!("{}", Extension("MOVE".to_string())));
    }

    #[test]
    fn test_hashable() {
        let mut counter: HashMap<Method,usize> = HashMap::new();
        counter.insert(Get, 1);
        assert_eq!(Some(&1), counter.get(&Get));
    }
}
