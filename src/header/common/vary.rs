use unicase::UniCase;

header! {
    #[doc="`Vary` header, defined in [RFC7231](https://tools.ietf.org/html/rfc7231#section-7.1.4)"]
    #[doc=""]
    #[doc="The \"Vary\" header field in a response describes what parts of a"]
    #[doc="request message, aside from the method, Host header field, and"]
    #[doc="request target, might influence the origin server's process for"]
    #[doc="selecting and representing this response.  The value consists of"]
    #[doc="either a single asterisk (\"*\") or a list of header field names"]
    #[doc="(case-insensitive)."]
    #[doc=""]
    #[doc="# ABNF"]
    #[doc="```plain"]
    #[doc="Vary = \"*\" / 1#field-name"]
    #[doc="```"]
    (Vary, "Vary") => {Any / (UniCase<String>)+}
}

/*/// The `Allow` header.
/// See also https://tools.ietf.org/html/rfc7231#section-7.1.4

#[derive(Clone, PartialEq, Debug)]
pub enum Vary {
    /// This corresponds to '*'.
    Any,
    /// The header field names which will influence the response representation.
    Headers(Vec<UniCase<String>>),
}

impl Header for Vary {
    fn header_name() -> &'static str {
        "Vary"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<Vary> {
        from_one_raw_str(raw).and_then(|s: String| {
            let slice = &s[..];
            match slice {
                "" => None,
                "*" => Some(Vary::Any),
                _ => from_comma_delimited(raw).map(|vec| Vary::Headers(vec)),
            }
        })
    }
}

impl HeaderFormat for Vary {
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Vary::Any => { write!(fmt, "*") }
            Vary::Headers(ref fields) => { fmt_comma_delimited(fmt, &fields[..]) }
        }
    }
}*/

#[cfg(test)]
mod tests {
    use super::Vary;
    use header::Header;

    #[test]
    fn test_vary() {
        let mut vary: Option<Vary>;

        vary = Header::parse_header([b"*".to_vec()].as_ref());
        assert_eq!(vary, Some(Vary::Any));

        vary = Header::parse_header([b"etag,cookie,allow".to_vec()].as_ref());
        assert_eq!(vary, Some(Vary::Items(vec!["eTag".parse().unwrap(),
                                                "cookIE".parse().unwrap(),
                                                "AlLOw".parse().unwrap(),])));
    }
}
