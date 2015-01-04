use header::shared;

#[deriving(Clone)]
enum AccessControlAllowOrigin {
    AllowStar,
    AllowOrigin(Url),
}

impl header::Header for AccessControlAllowOrigin {
    #[inline]
    fn header_name(_: Option<AccessControlAllowOrigin>) -> &'static str {
        "Access-Control-Allow-Origin"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<AccessControlAllowOrigin> {
        if raw.len() == 1 {
            from_utf8(raw[0].as_slice()).and_then(|s| {
                if s == "*" {
                    Some(AllowStar)
                } else {
                    Url::parse(s).ok().map(|url| AllowOrigin(url))
                }
            })
        } else {
            None
        }
    }
}

impl header::HeaderFormat for AccessControlAllowOrigin {
    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            AllowStar => "*".fmt(f),
            AllowOrigin(ref url) => url.fmt(f)
        }
    }
}
