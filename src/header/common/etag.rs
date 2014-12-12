use header::{Header, HeaderFormat};
use std::fmt::{mod};
use super::util::from_one_raw_str;

/// The `Etag` header.
///
/// An Etag consists of a string enclosed by two literal double quotes.
/// Preceding the first double quote is an optional weakness indicator,
/// which always looks like this: W/
/// See also: https://tools.ietf.org/html/rfc7232#section-2.3
#[deriving(Clone, PartialEq, Show)]
pub struct Etag {
    /// Weakness indicator for the tag
    pub weak: bool,
    /// The opaque string in between the DQUOTEs
    pub tag: String
}

impl Header for Etag {
    fn header_name(_: Option<Etag>) -> &'static str {
        "Etag"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<Etag> {
        // check that each char in the slice is either:
        // 1. %x21, or
        // 2. in the range %x23 to %x7E, or
        // 3. in the range %x80 to %xFF
        fn check_slice_validity(slice: &str) -> bool {
            for c in slice.bytes() {
                match c {
                    b'\x21' | b'\x23' ... b'\x7e' | b'\x80' ... b'\xff' => (),
                    _ => { return false; }
                }
            }
            true
        }


        from_one_raw_str(raw).and_then(|s: String| {
            let length: uint = s.len();
            let slice = s[];

            // Early exits:
            // 1. The string is empty, or,
            // 2. it doesn't terminate in a DQUOTE.
            if slice.is_empty() || !slice.ends_with("\"") {
                return None;
            }

            // The etag is weak if its first char is not a DQUOTE.
            if slice.char_at(0) == '"' {
                // No need to check if the last char is a DQUOTE,
                // we already did that above.
                if check_slice_validity(slice.slice_chars(1, length-1)) {
                    return Some(Etag {
                        weak: false,
                        tag: slice.slice_chars(1, length-1).into_string()
                    });
                } else {
                    return None;
                }
            }

            if slice.slice_chars(0, 3) == "W/\"" {
                if check_slice_validity(slice.slice_chars(3, length-1)) {
                    return Some(Etag {
                        weak: true,
                        tag: slice.slice_chars(3, length-1).into_string()
                    });
                } else {
                    return None;
                }
            }

            None
        })
    }
}

impl HeaderFormat for Etag {
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        if self.weak {
            try!(fmt.write(b"W/"));
        }
        write!(fmt, "\"{}\"", self.tag)
    }
}

#[cfg(test)]
mod tests {
    use super::Etag;
    use header::Header;

    #[test]
    fn test_etag_successes() {
        // Expected successes
        let mut etag: Option<Etag>;

        etag = Header::parse_header([b"\"foobar\"".to_vec()].as_slice());
        assert_eq!(etag, Some(Etag {
            weak: false,
            tag: "foobar".into_string()
        }));

        etag = Header::parse_header([b"\"\"".to_vec()].as_slice());
        assert_eq!(etag, Some(Etag {
            weak: false,
            tag: "".into_string()
        }));

        etag = Header::parse_header([b"W/\"weak-etag\"".to_vec()].as_slice());
        assert_eq!(etag, Some(Etag {
            weak: true,
            tag: "weak-etag".into_string()
        }));

        etag = Header::parse_header([b"W/\"\x65\x62\"".to_vec()].as_slice());
        assert_eq!(etag, Some(Etag {
            weak: true,
            tag: "\u{0065}\u{0062}".into_string()
        }));

        etag = Header::parse_header([b"W/\"\"".to_vec()].as_slice());
        assert_eq!(etag, Some(Etag {
            weak: true,
            tag: "".into_string()
        }));
    }

    #[test]
    fn test_etag_failures() {
        // Expected failures
        let mut etag: Option<Etag>;

        etag = Header::parse_header([b"no-dquotes".to_vec()].as_slice());
        assert_eq!(etag, None);

        etag = Header::parse_header([b"w/\"the-first-w-is-case-sensitive\"".to_vec()].as_slice());
        assert_eq!(etag, None);

        etag = Header::parse_header([b"".to_vec()].as_slice());
        assert_eq!(etag, None);

        etag = Header::parse_header([b"\"unmatched-dquotes1".to_vec()].as_slice());
        assert_eq!(etag, None);

        etag = Header::parse_header([b"unmatched-dquotes2\"".to_vec()].as_slice());
        assert_eq!(etag, None);

        etag = Header::parse_header([b"matched-\"dquotes\"".to_vec()].as_slice());
        assert_eq!(etag, None);
    }
}

bench_header!(bench, Etag, { vec![b"W/\"nonemptytag\"".to_vec()] })
