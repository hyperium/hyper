use std::str::FromStr;
use std::fmt::{self, Display};

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

/// An entity tag, defined in [RFC7232](https://tools.ietf.org/html/rfc7232#section-2.3)
///
/// An entity tag consists of a string enclosed by two literal double quotes.
/// Preceding the first double quote is an optional weakness indicator,
/// which always looks like `W/`. Examples for valid tags are `"xyzzy"` and `W/"xyzzy"`.
///
/// # ABNF
/// ```plain
/// entity-tag = [ weak ] opaque-tag
/// weak       = %x57.2F ; "W/", case-sensitive
/// opaque-tag = DQUOTE *etagc DQUOTE
/// etagc      = %x21 / %x23-7E / obs-text
///            ; VCHAR except double quotes, plus obs-text
/// ```
///
/// # Comparison
/// To check if two entity tags are equivalent in an application always use the `strong_eq` or
/// `weak_eq` methods based on the context of the Tag. Only use `==` to check if two tags are
/// identical.
///
/// The example below shows the results for a set of entity-tag pairs and
/// both the weak and strong comparison function results:
///
/// | ETag 1  | ETag 2  | Strong Comparison | Weak Comparison |
/// |---------|---------|-------------------|-----------------|
/// | `W/"1"` | `W/"1"` | no match          | match           |
/// | `W/"1"` | `W/"2"` | no match          | no match        |
/// | `W/"1"` | `"1"`   | no match          | match           |
/// | `"1"`   | `"1"`   | match             | match           |
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityTag {
    /// Weakness indicator for the tag
    pub weak: bool,
    /// The opaque string in between the DQUOTEs
    tag: String
}

impl EntityTag {
    /// Constructs a new EntityTag.
    /// # Panics
    /// If the tag contains invalid characters.
    pub fn new(weak: bool, tag: String) -> EntityTag {
        match check_slice_validity(&tag) {
            true => EntityTag { weak: weak, tag: tag },
            false => panic!("Invalid tag: {:?}", tag),
        }
    }

    /// Get the tag.
    pub fn tag(&self) -> &str {
        self.tag.as_ref()
    }

    /// Set the tag.
    /// # Panics
    /// If the tag contains invalid characters.
    pub fn set_tag(&mut self, tag: String) {
        match check_slice_validity(&tag[..]) {
            true => self.tag = tag,
            false => panic!("Invalid tag: {:?}", tag),
        }
    }

    /// For strong comparison two entity-tags are equivalent if both are not weak and their
    /// opaque-tags match character-by-character.
    pub fn strong_eq(&self, other: &EntityTag) -> bool {
        self.weak == false && other.weak == false && self.tag == other.tag
    }

    /// For weak comparison two entity-tags are equivalent if their
    /// opaque-tags match character-by-character, regardless of either or
    /// both being tagged as "weak".
    pub fn weak_eq(&self, other: &EntityTag) -> bool {
        self.tag == other.tag
    }

    /// The inverse of `EntityTag.strong_eq()`.
    pub fn strong_ne(&self, other: &EntityTag) -> bool {
        !self.strong_eq(other)
    }

    /// The inverse of `EntityTag.weak_eq()`.
    pub fn weak_ne(&self, other: &EntityTag) -> bool {
        !self.weak_eq(other)
    }
}

impl Display for EntityTag {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self.weak {
            true => write!(fmt, "W/\"{}\"", self.tag),
            false => write!(fmt, "\"{}\"", self.tag),
        }
    }
}

impl FromStr for EntityTag {
    type Err = ();
    fn from_str(s: &str) -> Result<EntityTag, ()> {
        let length: usize = s.len();
        let slice = &s[..];
        // Early exits:
        // 1. The string is empty, or,
        // 2. it doesn't terminate in a DQUOTE.
        if slice.is_empty() || !slice.ends_with('"') {
            return Err(());
        }
        // The etag is weak if its first char is not a DQUOTE.
        if slice.starts_with('"') && check_slice_validity(&slice[1..length-1]) {
            // No need to check if the last char is a DQUOTE,
            // we already did that above.
            return Ok(EntityTag { weak: false, tag: slice[1..length-1].to_string() });
        } else if slice.starts_with("W/\"") && check_slice_validity(&slice[3..length-1]) {
            return Ok(EntityTag { weak: true, tag: slice[3..length-1].to_string() });
        }
        Err(())
    }
}

#[cfg(test)]
mod tests {
    use super::EntityTag;

    #[test]
    fn test_etag_parse_success() {
        // Expected success
        assert_eq!("\"foobar\"".parse::<EntityTag>().unwrap(), EntityTag::new(false, "foobar".to_string()));
        assert_eq!("\"\"".parse::<EntityTag>().unwrap(), EntityTag::new(false, "".to_string()));
        assert_eq!("W/\"weaktag\"".parse::<EntityTag>().unwrap(), EntityTag::new(true, "weaktag".to_string()));
        assert_eq!("W/\"\x65\x62\"".parse::<EntityTag>().unwrap(), EntityTag::new(true, "\x65\x62".to_string()));
        assert_eq!("W/\"\"".parse::<EntityTag>().unwrap(), EntityTag::new(true, "".to_string()));
    }

    #[test]
    fn test_etag_parse_failures() {
        // Expected failures
        assert_eq!("no-dquotes".parse::<EntityTag>(), Err(()));
        assert_eq!("w/\"the-first-w-is-case-sensitive\"".parse::<EntityTag>(), Err(()));
        assert_eq!("".parse::<EntityTag>(), Err(()));
        assert_eq!("\"unmatched-dquotes1".parse::<EntityTag>(), Err(()));
        assert_eq!("unmatched-dquotes2\"".parse::<EntityTag>(), Err(()));
        assert_eq!("matched-\"dquotes\"".parse::<EntityTag>(), Err(()));
    }

    #[test]
    fn test_etag_fmt() {
        assert_eq!(format!("{}", EntityTag::new(false, "foobar".to_string())), "\"foobar\"");
        assert_eq!(format!("{}", EntityTag::new(false, "".to_string())), "\"\"");
        assert_eq!(format!("{}", EntityTag::new(true, "weak-etag".to_string())), "W/\"weak-etag\"");
        assert_eq!(format!("{}", EntityTag::new(true, "\u{0065}".to_string())), "W/\"\x65\"");
        assert_eq!(format!("{}", EntityTag::new(true, "".to_string())), "W/\"\"");
    }

    #[test]
    fn test_cmp() {
        // | ETag 1  | ETag 2  | Strong Comparison | Weak Comparison |
        // |---------|---------|-------------------|-----------------|
        // | `W/"1"` | `W/"1"` | no match          | match           |
        // | `W/"1"` | `W/"2"` | no match          | no match        |
        // | `W/"1"` | `"1"`   | no match          | match           |
        // | `"1"`   | `"1"`   | match             | match           |
        let mut etag1 = EntityTag::new(true, "1".to_string());
        let mut etag2 = EntityTag::new(true, "1".to_string());
        assert_eq!(etag1.strong_eq(&etag2), false);
        assert_eq!(etag1.weak_eq(&etag2), true);
        assert_eq!(etag1.strong_ne(&etag2), true);
        assert_eq!(etag1.weak_ne(&etag2), false);

        etag1 = EntityTag::new(true, "1".to_string());
        etag2 = EntityTag::new(true, "2".to_string());
        assert_eq!(etag1.strong_eq(&etag2), false);
        assert_eq!(etag1.weak_eq(&etag2), false);
        assert_eq!(etag1.strong_ne(&etag2), true);
        assert_eq!(etag1.weak_ne(&etag2), true);

        etag1 = EntityTag::new(true, "1".to_string());
        etag2 = EntityTag::new(false, "1".to_string());
        assert_eq!(etag1.strong_eq(&etag2), false);
        assert_eq!(etag1.weak_eq(&etag2), true);
        assert_eq!(etag1.strong_ne(&etag2), true);
        assert_eq!(etag1.weak_ne(&etag2), false);

        etag1 = EntityTag::new(false, "1".to_string());
        etag2 = EntityTag::new(false, "1".to_string());
        assert_eq!(etag1.strong_eq(&etag2), true);
        assert_eq!(etag1.weak_eq(&etag2), true);
        assert_eq!(etag1.strong_ne(&etag2), false);
        assert_eq!(etag1.weak_ne(&etag2), false);
    }
}
