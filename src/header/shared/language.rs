use std::str::FromStr;
use std::fmt;

/// A language tag.
/// See http://www.w3.org/Protocols/rfc2616/rfc2616-sec3.html#sec3.10
///
/// Note: This is no complete language tag implementation, it should be replaced with
/// github.com/pyfisch/rust-language-tag once it is ready.
#[derive(Clone, PartialEq, Debug)]
pub struct Language {
    /// The language tag
    pub primary: String,
    /// A language subtag or country code
    pub sub: Option<String>
}

impl FromStr for Language {
    type Err = ();
    fn from_str(s: &str) -> Result<Language, ()> {
        let mut i = s.split("-");
        let p = i.next();
        let s = i.next();
        match (p, s) {
            (Some(p), Some(s)) => Ok(Language {
                primary: p.to_owned(),
                sub: Some(s.to_owned())
                }),
            (Some(p), _) => Ok(Language {
                primary: p.to_owned(),
                sub: None
                }),
            _ => Err(())
        }
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(f.write_str(&self.primary[..]));
        match self.sub {
            Some(ref s) => write!(f, "-{}", s),
            None => Ok(())
        }
    }
}
