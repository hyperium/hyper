use std::ascii::AsciiExt;
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use url;

use header::HttpDate;

// Copied from https://github.com/alexcrichton/cookie-rs

/// A single HTTP cookie
#[derive(PartialEq, Clone, Debug)]
pub struct CookiePair {
    /// The name, or key of the cookie
    pub name: String,
    /// The value of the cookie
    pub value: String,
    /// The date the cookie should not be sent after
    pub expires: Option<HttpDate>,
    /// The time from now in seconds the cookie expires
    // FIXME: Use duration?
    pub max_age: Option<u64>,
    /// The domain the cookie is valid for
    // FIXME: Use more specific type
    pub domain: Option<String>,
    /// The domain path the cookie is valid for
    // FIXME: Use more specific type
    pub path: Option<String>,
    /// `true` if the cookie should only get transmitted over secure connections
    pub secure: bool,
    /// `true` if the cookie should only get transmitted over HTTP connections
    pub httponly: bool,
    /// custom extension attributes
    pub custom: BTreeMap<String, String>,
}


impl CookiePair {
    /// Creates a new cookie-pair, with a given name and value
    ///
    /// The other optional arguments are set to default values.
    pub fn new(name: String, value: String) -> CookiePair {
        CookiePair {
            name: name,
            value: value,
            expires: None,
            max_age: None,
            domain: None,
            path: Some("/".to_string()),
            secure: false,
            httponly: false,
            custom: BTreeMap::new(),
        }
    }

    /// Returns the name-value part of the cookie
    pub fn pair(&self) -> AttrVal {
        AttrVal(&self.name, &self.value)
    }
}

/// Struct of the name-value part of the cookie, excluding arguments
pub struct AttrVal<'a>(pub &'a str, pub &'a str);

impl<'a> fmt::Display for AttrVal<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let AttrVal(ref attr, ref val) = *self;
        write!(f, "{}={}", attr, url::percent_encode(val.as_bytes(),
                                                     url::DEFAULT_ENCODE_SET))
    }
}

impl fmt::Display for CookiePair {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(AttrVal(&self.name, &self.value).fmt(f));
        if self.httponly { try!(write!(f, "; HttpOnly")); }
        if self.secure { try!(write!(f, "; Secure")); }
        match self.path {
            Some(ref s) => try!(write!(f, "; Path={}", s)),
            None => {}
        }
        match self.domain {
            Some(ref s) => try!(write!(f, "; Domain={}", s)),
            None => {}
        }
        match self.max_age {
            Some(n) => try!(write!(f, "; Max-Age={}", n)),
            None => {}
        }
        match self.expires {
            Some(ref t) => try!(write!(f, "; Expires={}", t)),
            None => {}
        }

        for (k, v) in self.custom.iter() {
            try!(write!(f, "; {}", AttrVal(&k, &v)));
        }
        Ok(())
    }
}

impl FromStr for CookiePair {
    type Err = ();
    fn from_str(s: &str) -> Result<CookiePair, ()> {
        macro_rules! unwrap_or_skip{ ($e:expr) => (
            match $e { Some(s) => s, None => continue, }
        ) }

        let mut c = CookiePair::new(String::new(), String::new());
        let mut pairs = s.trim().split(';');
        let keyval = match pairs.next() { Some(s) => s, _ => return Err(()) };
        let (name, value) = try!(split(keyval));
        let name = url::percent_decode(name.as_bytes());
        if name.is_empty() {
            return Err(());
        }
        let value = url::percent_decode(value.as_bytes());
        c.name = try!(String::from_utf8(name).map_err(|_| ()));
        c.value = try!(String::from_utf8(value).map_err(|_| ()));

        for attr in pairs {
            let trimmed = attr.trim();
            match &trimmed.to_ascii_lowercase()[..] {
                "secure" => c.secure = true,
                "httponly" => c.httponly = true,
                _ => {
                    let (k, v) = unwrap_or_skip!(split(trimmed).ok());
                    match &k.to_ascii_lowercase()[..] {
                        "max-age" => c.max_age = Some(unwrap_or_skip!(v.parse().ok())),
                        "domain" => {
                            if v.is_empty() {
                                continue;
                            }

                            let domain = if v.chars().next() == Some('.') {
                                &v[1..]
                            } else {
                                v
                            };
                            c.domain = Some(domain.to_ascii_lowercase());
                        }
                        "path" => c.path = Some(v.to_string()),
                        "expires" => {
                            match v.parse() {
                                Ok(date) => c.expires = Some(date),
                                Err(_) => {}
                            }
                        }
                        _ => { c.custom.insert(k.to_string(), v.to_string()); }
                    }
                }
            }
        }

        return Ok(c);

        fn split<'a>(s: &'a str) -> Result<(&'a str, &'a str), ()> {
            macro_rules! try {
                ($e:expr) => (match $e { Some(s) => s, None => return Err(()) })
            }
            let mut parts = s.trim().splitn(2, '=');
            let first = try!(parts.next()).trim();
            let second = try!(parts.next()).trim();
            Ok((first, second))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CookiePair;

    #[test]
    fn parse() {
        assert!("bar".parse::<CookiePair>().is_err());
        assert!("=bar".parse::<CookiePair>().is_err());
        assert!(" =bar".parse::<CookiePair>().is_err());
        assert!("foo=".parse::<CookiePair>().is_ok());
        let mut expected = CookiePair::new("foo".to_string(), "bar".to_string());
        assert_eq!("foo=bar".parse::<CookiePair>().ok().unwrap(), expected);
        assert_eq!("foo = bar".parse::<CookiePair>().ok().unwrap(), expected);
        assert_eq!("foo=bar ".parse::<CookiePair>().ok().unwrap(), expected);
        assert_eq!("foo=bar ;Domain=".parse::<CookiePair>().ok().unwrap(), expected);
        assert_eq!("foo=bar ;Domain= ".parse::<CookiePair>().ok().unwrap(), expected);
        assert_eq!("foo=bar ;Ignored".parse::<CookiePair>().ok().unwrap(), expected);
        expected.httponly = true;
        assert_eq!("foo=bar ;HttpOnly".parse::<CookiePair>().ok().unwrap(), expected);
        assert_eq!("foo=bar ;httponly".parse::<CookiePair>().ok().unwrap(), expected);
        assert_eq!("foo=bar ;HTTPONLY".parse::<CookiePair>().ok().unwrap(), expected);
        assert_eq!("foo=bar ; sekure; HTTPONLY".parse::<CookiePair>().ok().unwrap(), expected);
        expected.secure = true;
        assert_eq!("foo=bar ;HttpOnly; Secure".parse::<CookiePair>().ok().unwrap(), expected);
        expected.max_age = Some(4);
        assert_eq!("foo=bar ;HttpOnly; Secure; \
                    Max-Age=4".parse::<CookiePair>().ok().unwrap(), expected);
        assert_eq!("foo=bar ;HttpOnly; Secure; \
                    Max-Age = 4 ".parse::<CookiePair>().ok().unwrap(), expected);
        expected.path = Some("/foo".to_string());
        assert_eq!("foo=bar ;HttpOnly; Secure; \
                    Max-Age=4; Path=/foo".parse::<CookiePair>().ok().unwrap(), expected);
        expected.domain = Some("foo.com".to_string());
        assert_eq!("foo=bar ;HttpOnly; Secure; \
                    Max-Age=4; Path=/foo; \
                    Domain=foo.com".parse::<CookiePair>().ok().unwrap(), expected);
        assert_eq!("foo=bar ;HttpOnly; Secure; \
                    Max-Age=4; Path=/foo; \
                    Domain=FOO.COM".parse::<CookiePair>().ok().unwrap(), expected);
        expected.custom.insert("wut".to_string(), "lol".to_string());
        assert_eq!("foo=bar ;HttpOnly; Secure; \
                    Max-Age=4; Path=/foo; \
                    Domain=foo.com; wut=lol".parse::<CookiePair>().ok().unwrap(), expected);

        assert_eq!(expected.to_string(),
                   "foo=bar; HttpOnly; Secure; Path=/foo; Domain=foo.com; \
                    Max-Age=4; wut=lol");
    }

    #[test]
    fn odd_characters() {
        let expected = CookiePair::new("foo".to_string(), "b/r".to_string());
        assert_eq!("foo=b%2Fr".parse::<CookiePair>().ok().unwrap(), expected);
    }

    #[test]
    fn pair() {
        let cookie = CookiePair::new("foo".to_string(), "bar".to_string());
        assert_eq!(cookie.pair().to_string(), "foo=bar".to_string());
    }
}
