use method::Method;

header! {
    #[doc="`Allow` header, defined in [RFC7231](http://tools.ietf.org/html/rfc7231#section-7.4.1)"]
    #[doc=""]
    #[doc="The `Allow` header field lists the set of methods advertised as"]
    #[doc="supported by the target resource.  The purpose of this field is"]
    #[doc="strictly to inform the recipient of valid request methods associated"]
    #[doc="with the resource."]
    #[doc=""]
    #[doc="# ABNF"]
    #[doc="```plain"]
    #[doc="Allow = #method"]
    #[doc="```"]
    (Allow, "Allow") => (Method)*
}

#[cfg(test)]
mod tests {
    use super::Allow;
    use header::Header;
    use method::Method::{self, Options, Get, Put, Post, Delete, Head, Trace, Connect, Patch, Extension};

    #[test]
    fn test_allow() {
        let mut allow: Option<Allow>;

        allow = Header::parse_header([b"OPTIONS,GET,PUT,POST,DELETE,HEAD,TRACE,CONNECT,PATCH,fOObAr".to_vec()].as_ref());
        assert_eq!(allow, Some(Allow(vec![Options, Get, Put, Post, Delete, Head, Trace, Connect, Patch, Extension("fOObAr".to_string())])));

        allow = Header::parse_header([b"".to_vec()].as_ref());
        assert_eq!(allow, Some(Allow(Vec::<Method>::new())));
    }
}

bench_header!(bench, Allow, { vec![b"OPTIONS,GET,PUT,POST,DELETE,HEAD,TRACE,CONNECT,PATCH,fOObAr".to_vec()] });
