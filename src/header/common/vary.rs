use header::{Header, HeaderFormat};
use std::fmt::{self};
use header::parsing::{from_comma_delimited, fmt_comma_delimited, from_one_raw_str};
use unicase::UniCase;

/// The `Allow` header.
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
            let slice = &s[];
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
            Vary::Headers(ref fields) => { fmt_comma_delimited(fmt, &fields[]) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Vary;
    use header::Header;

    #[test]
    fn test_vary() {
        let mut vary: Option<Vary>;

        vary = Header::parse_header([b"*".to_vec()].as_slice());
        assert_eq!(vary, Some(Vary::Any));

        vary = Header::parse_header([b"etag,cookie,allow".to_vec()].as_slice());
        assert_eq!(vary, Some(Vary::Headers(vec!["eTag".parse().unwrap(),
                                                 "cookIE".parse().unwrap(),
                                                 "AlLOw".parse().unwrap(),])));
    }
}
