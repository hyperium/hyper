//! The HTTP request method
use std::fmt;
use std::str::FromStr;
use std::convert::AsRef;

#[cfg(feature = "compat")]
use http_types;

use error::Error;
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
    /// Method extensions. An example would be `let m = Extension("FOO".to_string())`.
    Extension(String)
}

impl AsRef<str> for Method {
    fn as_ref(&self) -> &str {
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
            Extension(ref s) => s.as_ref()
        }
    }
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
    /// the same result if executed multiple times.
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

macro_rules! from_str {
    ($s:ident, { $($n:pat => { $($text:pat => $var:ident,)* },)* }) => ({
        let s = $s;
        match s.len() {
            $(
            $n => match s {
                $(
                $text => return Ok($var),
                )*
                _ => {},
            },
            )*
            0 => return Err(::Error::Method),
            _ => {},
        }
        Ok(Extension(s.to_owned()))
    })
}

impl FromStr for Method {
    type Err = Error;
    fn from_str(s: &str) -> Result<Method, Error> {
        from_str!(s, {
            3 => {
                "GET" => Get,
                "PUT" => Put,
            },
            4 => {
                "HEAD" => Head,
                "POST" => Post,
            },
            5 => {
                "PATCH" => Patch,
                "TRACE" => Trace,
            },
            6 => {
                "DELETE" => Delete,
            },
            7 => {
                "OPTIONS" => Options,
                "CONNECT" => Connect,
            },
        })
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
            Extension(ref s) => s.as_ref()
        })
    }
}

impl Default for Method {
    fn default() -> Method {
        Method::Get
    }
}

#[cfg(feature = "compat")]
impl From<http_types::Method> for Method {
    fn from(method: http_types::Method) -> Method {
        match method {
            http_types::Method::GET =>
                Method::Get,
            http_types::Method::POST =>
                Method::Post,
            http_types::Method::PUT =>
                Method::Put,
            http_types::Method::DELETE =>
                Method::Delete,
            http_types::Method::HEAD =>
                Method::Head,
            http_types::Method::OPTIONS =>
                Method::Options,
            http_types::Method::CONNECT =>
                Method::Connect,
            http_types::Method::PATCH =>
                Method::Patch,
            http_types::Method::TRACE =>
                Method::Trace,
            _ => {
                method.as_ref().parse()
                    .expect("attempted to convert invalid method")
            }
        }
    }
}

#[cfg(feature = "compat")]
impl From<Method> for http_types::Method {
    fn from(method: Method) -> http_types::Method {
        use http_types::HttpTryFrom;

        match method {
            Method::Get =>
                http_types::Method::GET,
            Method::Post =>
                http_types::Method::POST,
            Method::Put =>
                http_types::Method::PUT,
            Method::Delete =>
                http_types::Method::DELETE,
            Method::Head =>
                http_types::Method::HEAD,
            Method::Options =>
                http_types::Method::OPTIONS,
            Method::Connect =>
                http_types::Method::CONNECT,
            Method::Patch =>
                http_types::Method::PATCH,
            Method::Trace =>
                http_types::Method::TRACE,
            Method::Extension(s) => {
                HttpTryFrom::try_from(s.as_str())
                    .expect("attempted to convert invalid method")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::str::FromStr;
    use error::Error;
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
        assert_eq!(Get, FromStr::from_str("GET").unwrap());
        assert_eq!(Extension("MOVE".to_owned()),
                   FromStr::from_str("MOVE").unwrap());
        let x: Result<Method, _> = FromStr::from_str("");
        if let Err(Error::Method) = x {
        } else {
            panic!("An empty method is invalid!")
        }
    }

    #[test]
    fn test_fmt() {
        assert_eq!("GET".to_owned(), format!("{}", Get));
        assert_eq!("MOVE".to_owned(),
                   format!("{}", Extension("MOVE".to_owned())));
    }

    #[test]
    fn test_hashable() {
        let mut counter: HashMap<Method,usize> = HashMap::new();
        counter.insert(Get, 1);
        assert_eq!(Some(&1), counter.get(&Get));
    }

    #[test]
    fn test_as_str() {
        assert_eq!(Get.as_ref(), "GET");
        assert_eq!(Post.as_ref(), "POST");
        assert_eq!(Put.as_ref(), "PUT");
        assert_eq!(Extension("MOVE".to_owned()).as_ref(), "MOVE");
    }

    #[test]
    #[cfg(feature = "compat")]
    fn test_compat() {
        use http_types::{self, HttpTryFrom};

        let methods = vec![
            "GET",
            "POST",
            "PUT",
            "MOVE"
        ];
        for method in methods {
            let orig_hyper_method = Method::from_str(method).unwrap();
            let orig_http_method = http_types::Method::try_from(method).unwrap();
            let conv_hyper_method: Method = orig_http_method.clone().into();
            let conv_http_method: http_types::Method = orig_hyper_method.clone().into();
            assert_eq!(orig_hyper_method, conv_hyper_method);
            assert_eq!(orig_http_method, conv_http_method);
        }
    }
}
