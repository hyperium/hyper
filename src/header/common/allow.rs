use method::Method;

/// The `Allow` header.
/// See also https://tools.ietf.org/html/rfc7231#section-7.4.1

#[derive(Clone, PartialEq, Debug)]
pub struct Allow(pub Vec<Method>);

impl_list_header!(Allow,
                  "Allow",
                  Vec<Method>);

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
