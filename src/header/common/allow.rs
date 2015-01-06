use header::{Header, HeaderFormat};
use method::Method;
use std::fmt::{self};
use header::shared::util::{from_comma_delimited, fmt_comma_delimited};

/// The `Allow` header.
/// See also https://tools.ietf.org/html/rfc7231#section-7.4.1

#[derive(Clone, PartialEq, Show)]
pub struct Allow(pub Vec<Method>);

deref!(Allow -> Vec<Method>);

impl Header for Allow {
    fn header_name(_: Option<Allow>) -> &'static str {
        "Allow"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<Allow> {
        from_comma_delimited(raw).map(|vec| Allow(vec))
    }
}

impl HeaderFormat for Allow {
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt_comma_delimited(fmt, self[])
    }
}

#[cfg(test)]
mod tests {
    use super::Allow;
    use header::Header;
    use method::Method::{self, Options, Get, Put, Post, Delete, Head, Trace, Connect, Patch, Extension};

    #[test]
    fn test_allow() {
        let mut allow: Option<Allow>;

        allow = Header::parse_header([b"OPTIONS,GET,PUT,POST,DELETE,HEAD,TRACE,CONNECT,PATCH,fOObAr".to_vec()].as_slice());
        assert_eq!(allow, Some(Allow(vec![Options, Get, Put, Post, Delete, Head, Trace, Connect, Patch, Extension("fOObAr".to_string())])));

        allow = Header::parse_header([b"".to_vec()].as_slice());
        assert_eq!(allow, Some(Allow(Vec::<Method>::new())));
    }
}

bench_header!(bench, Allow, { vec![b"OPTIONS,GET,PUT,POST,DELETE,HEAD,TRACE,CONNECT,PATCH,fOObAr".to_vec()] });
