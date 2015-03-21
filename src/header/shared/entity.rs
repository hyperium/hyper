use std::str::FromStr;
use std::fmt::{self, Display};

/// An entity tag
///
/// An Etag consists of a string enclosed by two literal double quotes.
/// Preceding the first double quote is an optional weakness indicator,
/// which always looks like this: W/
/// See also: https://tools.ietf.org/html/rfc7232#section-2.3
#[derive(Clone, PartialEq, Debug)]
pub struct EntityTag {
    /// Weakness indicator for the tag
    pub weak: bool,
    /// The opaque string in between the DQUOTEs
    pub tag: String
}

impl Display for EntityTag {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        if self.weak {
            try!(write!(fmt, "{}", "W/"));
        }
        write!(fmt, "{}", self.tag)
    }
}

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

impl FromStr for EntityTag {
    type Err = ();
    fn from_str(s: &str) -> Result<EntityTag, ()> {
        let length: usize = s.len();
        let slice = &s[..];

        // Early exits:
        // 1. The string is empty, or,
        // 2. it doesn't terminate in a DQUOTE.
        if slice.is_empty() || !slice.ends_with("\"") {
            return Err(());
        }

        // The etag is weak if its first char is not a DQUOTE.
        if slice.chars().next().unwrap() == '"' /* '"' */ {
            // No need to check if the last char is a DQUOTE,
            // we already did that above.
            if check_slice_validity(slice.slice_chars(1, length-1)) {
                return Ok(EntityTag {
                    weak: false,
                    tag: slice.slice_chars(1, length-1).to_string()
                });
            } else {
                return Err(());
            }
        }

        if slice.slice_chars(0, 3) == "W/\"" {
            if check_slice_validity(slice.slice_chars(3, length-1)) {
                return Ok(EntityTag {
                    weak: true,
                    tag: slice.slice_chars(3, length-1).to_string()
                });
            } else {
                return Err(());
            }
        }

        Err(())
    }
}


#[cfg(test)]
mod tests {
    use super::EntityTag;

    #[test]
    fn test_etag_successes() {
        // Expected successes
        let mut etag : EntityTag = "\"foobar\"".parse().unwrap();
        assert_eq!(etag, (EntityTag {
            weak: false,
            tag: "foobar".to_string()
        }));

        etag = "\"\"".parse().unwrap();
        assert_eq!(etag, EntityTag {
            weak: false,
            tag: "".to_string()
        });

        etag = "W/\"weak-etag\"".parse().unwrap();
        assert_eq!(etag, EntityTag {
            weak: true,
            tag: "weak-etag".to_string()
        });

        etag = "W/\"\x65\x62\"".parse().unwrap();
        assert_eq!(etag, EntityTag {
            weak: true,
            tag: "\u{0065}\u{0062}".to_string()
        });

        etag = "W/\"\"".parse().unwrap();
        assert_eq!(etag, EntityTag {
            weak: true,
            tag: "".to_string()
        });
    }

    #[test]
    fn test_etag_failures() {
        // Expected failures
        let mut etag: Result<EntityTag,()>;

        etag = "no-dquotes".parse();
        assert_eq!(etag, Err(()));

        etag = "w/\"the-first-w-is-case-sensitive\"".parse();
        assert_eq!(etag, Err(()));

        etag = "".parse();
        assert_eq!(etag, Err(()));

        etag = "\"unmatched-dquotes1".parse();
        assert_eq!(etag, Err(()));

        etag = "unmatched-dquotes2\"".parse();
        assert_eq!(etag, Err(()));

        etag = "matched-\"dquotes\"".parse();
        assert_eq!(etag, Err(()));
    }
}
