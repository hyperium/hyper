use std::fmt;
use std::str::FromStr;
use header::{Header, HeaderFormat};
use header::parsing::{from_comma_delimited, fmt_comma_delimited};

/// `Prefer` header, defined in [RFC7240](http://tools.ietf.org/html/rfc7240)
///
/// The `Prefer` header field is HTTP header field that can be used by a
/// client to request that certain behaviors be employed by a server
/// while processing a request.
///
/// # ABNF
/// ```plain
/// Prefer     = "Prefer" ":" 1#preference
/// preference = token [ BWS "=" BWS word ]
///              *( OWS ";" [ OWS parameter ] )
/// parameter  = token [ BWS "=" BWS word ]
/// ```
///
/// # Example values
/// * `respond-async`
/// * `return=minimal`
/// * `wait=30`
///
/// # Examples
/// ```
/// use hyper::header::{Headers, Prefer, Preference};
///
/// let mut headers = Headers::new();
/// headers.set(
///     Prefer(vec![Preference::RespondAsync])
/// );
/// ```
/// ```
/// use hyper::header::{Headers, Prefer, Preference};
///
/// let mut headers = Headers::new();
/// headers.set(
///     Prefer(vec![
///         Preference::RespondAsync,
///         Preference::ReturnRepresentation,
///         Preference::Wait(10u32),
///         Preference::Extension("foo".to_owned(),
///                               "bar".to_owned(),
///                               vec![]),
///     ])
/// );
/// ```
#[derive(PartialEq, Clone, Debug)]
pub struct Prefer(pub Vec<Preference>);

__hyper__deref!(Prefer => Vec<Preference>);

impl Header for Prefer {
    fn header_name() -> &'static str {
        "Prefer"
    }

    fn parse_header(raw: &[Vec<u8>]) -> ::Result<Prefer> {
        let preferences = try!(from_comma_delimited(raw));
        if !preferences.is_empty() {
            Ok(Prefer(preferences))
        } else {
            Err(::Error::Header)
        }
    }
}

impl HeaderFormat for Prefer {
    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for Prefer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt_comma_delimited(f, &self[..])
    }
}

/// Prefer contains a list of these preferences.
#[derive(PartialEq, Clone, Debug)]
pub enum Preference {
    /// "respond-async"
    RespondAsync,
    /// "return=representation"
    ReturnRepresentation,
    /// "return=minimal"
    ReturnMinimal,
    /// "handling=strict"
    HandlingStrict,
    /// "handling=leniant"
    HandlingLeniant,
    /// "wait=delta"
    Wait(u32),

    /// Extension preferences. Always has a value, if none is specified it is
    /// just "". A preference can also have a list of parameters.
    Extension(String, String, Vec<(String, String)>)
}

impl fmt::Display for Preference {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Preference::*;
        fmt::Display::fmt(match *self {
            RespondAsync => "respond-async",
            ReturnRepresentation => "return=representation",
            ReturnMinimal => "return=minimal",
            HandlingStrict => "handling=strict",
            HandlingLeniant => "handling=leniant",

            Wait(secs) => return write!(f, "wait={}", secs),

            Extension(ref name, ref value, ref params) => {
                try!(write!(f, "{}", name));
                if value != "" { try!(write!(f, "={}", value)); }
                if params.len() > 0 {
                    for &(ref name, ref value) in params {
                        try!(write!(f, "; {}", name));
                        if value != "" { try!(write!(f, "={}", value)); }
                    }
                }
                return Ok(());
            }
        }, f)
    }
}

impl FromStr for Preference {
    type Err = Option<<u32 as FromStr>::Err>;
    fn from_str(s: &str) -> Result<Preference, Option<<u32 as FromStr>::Err>> {
        use self::Preference::*;
        let mut params = s.split(';').map(|p| {
            let mut param = p.splitn(2, '=');
            match (param.next(), param.next()) {
                (Some(name), Some(value)) => (name.trim(), value.trim().trim_matches('"')),
                (Some(name), None) => (name.trim(), ""),
                // This can safely be unreachable because the [`splitn`][1]
                // function (used above) will always have at least one value.
                //
                // [1]: http://doc.rust-lang.org/std/primitive.str.html#method.splitn
                _ => { unreachable!(); }
            }
        });
        match params.nth(0) {
            Some(param) => {
                let rest: Vec<(String, String)> = params.map(|(l, r)| (l.to_owned(), r.to_owned())).collect();
                match param {
                    ("respond-async", "") => if rest.len() == 0 { Ok(RespondAsync) } else { Err(None) },
                    ("return", "representation") => if rest.len() == 0 { Ok(ReturnRepresentation) } else { Err(None) },
                    ("return", "minimal") => if rest.len() == 0 { Ok(ReturnMinimal) } else { Err(None) },
                    ("handling", "strict") => if rest.len() == 0 { Ok(HandlingStrict) } else { Err(None) },
                    ("handling", "leniant") => if rest.len() == 0 { Ok(HandlingLeniant) } else { Err(None) },
                    ("wait", secs) => if rest.len() == 0 { secs.parse().map(Wait).map_err(Some) } else { Err(None) },
                    (left, right) => Ok(Extension(left.to_owned(), right.to_owned(), rest))
                }
            },
            None => Err(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use header::Header;
    use super::*;

    #[test]
    fn test_parse_multiple_headers() {
        let prefer = Header::parse_header(&[b"respond-async, return=representation".to_vec()]);
        assert_eq!(prefer.ok(), Some(Prefer(vec![Preference::RespondAsync,
                                           Preference::ReturnRepresentation])))
    }

    #[test]
    fn test_parse_argument() {
        let prefer = Header::parse_header(&[b"wait=100, handling=leniant, respond-async".to_vec()]);
        assert_eq!(prefer.ok(), Some(Prefer(vec![Preference::Wait(100),
                                           Preference::HandlingLeniant,
                                           Preference::RespondAsync])))
    }

    #[test]
    fn test_parse_quote_form() {
        let prefer = Header::parse_header(&[b"wait=\"200\", handling=\"strict\"".to_vec()]);
        assert_eq!(prefer.ok(), Some(Prefer(vec![Preference::Wait(200),
                                           Preference::HandlingStrict])))
    }

    #[test]
    fn test_parse_extension() {
        let prefer = Header::parse_header(&[b"foo, bar=baz, baz; foo; bar=baz, bux=\"\"; foo=\"\", buz=\"some parameter\"".to_vec()]);
        assert_eq!(prefer.ok(), Some(Prefer(vec![
            Preference::Extension("foo".to_owned(), "".to_owned(), vec![]),
            Preference::Extension("bar".to_owned(), "baz".to_owned(), vec![]),
            Preference::Extension("baz".to_owned(), "".to_owned(), vec![("foo".to_owned(), "".to_owned()), ("bar".to_owned(), "baz".to_owned())]),
            Preference::Extension("bux".to_owned(), "".to_owned(), vec![("foo".to_owned(), "".to_owned())]),
            Preference::Extension("buz".to_owned(), "some parameter".to_owned(), vec![])])))
    }

    #[test]
    fn test_fail_with_args() {
        let prefer: ::Result<Prefer> = Header::parse_header(&[b"respond-async; foo=bar".to_vec()]);
        assert_eq!(prefer.ok(), None);
    }
}

bench_header!(normal,
    Prefer, { vec![b"respond-async, return=representation".to_vec(), b"wait=100".to_vec()] });
