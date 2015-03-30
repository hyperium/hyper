use header::{EntityTag, Header, HeaderFormat};
use std::fmt::{self, Display};
use header::parsing::from_one_raw_str;

/// The `Etag` header.
///
/// An Etag consists of a string enclosed by two literal double quotes.
/// Preceding the first double quote is an optional weakness indicator,
/// which always looks like this: W/
/// See also: https://tools.ietf.org/html/rfc7232#section-2.3
#[derive(Clone, PartialEq, Debug)]
pub struct Etag(pub EntityTag);

deref!(Etag => EntityTag);

impl Header for Etag {
    fn header_name() -> &'static str {
        "Etag"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<Etag> {

        from_one_raw_str(raw).and_then(|s: String| {
            s.parse::<EntityTag>().and_then(|x| Ok(Etag(x))).ok()
        })
    }
}

impl HeaderFormat for Etag {
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(fmt)
    }
}

#[cfg(test)]
mod tests {
    use super::Etag;
    use header::{Header,EntityTag};

    #[test]
    fn test_etag_successes() {
        // Expected successes
        let mut etag: Option<Etag>;

        etag = Header::parse_header([b"\"foobar\"".to_vec()].as_ref());
        assert_eq!(etag, Some(Etag(EntityTag::new(false, "foobar".to_string()))));

        etag = Header::parse_header([b"\"\"".to_vec()].as_ref());
        assert_eq!(etag, Some(Etag(EntityTag::new(false, "".to_string()))));

        etag = Header::parse_header([b"W/\"weak-etag\"".to_vec()].as_ref());
        assert_eq!(etag, Some(Etag(EntityTag::new(true, "weak-etag".to_string()))));

        etag = Header::parse_header([b"W/\"\x65\x62\"".to_vec()].as_ref());
        assert_eq!(etag, Some(Etag(EntityTag::new(true, "\u{0065}\u{0062}".to_string()))));

        etag = Header::parse_header([b"W/\"\"".to_vec()].as_ref());
        assert_eq!(etag, Some(Etag(EntityTag::new(true, "".to_string()))));
    }

    #[test]
    fn test_etag_failures() {
        // Expected failures
        let mut etag: Option<Etag>;

        etag = Header::parse_header([b"no-dquotes".to_vec()].as_ref());
        assert_eq!(etag, None);

        etag = Header::parse_header([b"w/\"the-first-w-is-case-sensitive\"".to_vec()].as_ref());
        assert_eq!(etag, None);

        etag = Header::parse_header([b"".to_vec()].as_ref());
        assert_eq!(etag, None);

        etag = Header::parse_header([b"\"unmatched-dquotes1".to_vec()].as_ref());
        assert_eq!(etag, None);

        etag = Header::parse_header([b"unmatched-dquotes2\"".to_vec()].as_ref());
        assert_eq!(etag, None);

        etag = Header::parse_header([b"matched-\"dquotes\"".to_vec()].as_ref());
        assert_eq!(etag, None);
    }
}

bench_header!(bench, Etag, { vec![b"W/\"nonemptytag\"".to_vec()] });
