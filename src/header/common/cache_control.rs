use std::fmt;
use std::str::FromStr;
use header::{Header, HeaderFormat};
use header::parsing::{from_one_comma_delimited, fmt_comma_delimited};

/// The Cache-Control header.
#[derive(PartialEq, Clone, Debug)]
pub struct CacheControl(pub Vec<CacheDirective>);

deref!(CacheControl => Vec<CacheDirective>);

impl Header for CacheControl {
    fn header_name() -> &'static str {
        "Cache-Control"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<CacheControl> {
        let directives = raw.iter()
            .filter_map(|line| from_one_comma_delimited(&line[..]))
            .collect::<Vec<Vec<CacheDirective>>>()
            .concat();
        if !directives.is_empty() {
            Some(CacheControl(directives))
        } else {
            None
        }
    }
}

impl HeaderFormat for CacheControl {
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt_comma_delimited(fmt, &self[..])
    }
}

/// CacheControl contains a list of these directives.
#[derive(PartialEq, Clone, Debug)]
pub enum CacheDirective {
    /// "no-cache"
    NoCache,
    /// "no-store"
    NoStore,
    /// "no-transform"
    NoTransform,
    /// "only-if-cached"
    OnlyIfCached,

    // request directives
    /// "max-age=delta"
    MaxAge(u32),
    /// "max-stale=delta"
    MaxStale(u32),
    /// "min-fresh=delta"
    MinFresh(u32),

    // response directives
    /// "must-revalidate"
    MustRevalidate,
    /// "public"
    Public,
    /// "private"
    Private,
    /// "proxy-revalidate"
    ProxyRevalidate,
    /// "s-maxage=delta"
    SMaxAge(u32),

    /// Extension directives. Optionally include an argument.
    Extension(String, Option<String>)
}

impl fmt::Display for CacheDirective {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::CacheDirective::*;
        fmt::Display::fmt(match *self {
            NoCache => "no-cache",
            NoStore => "no-store",
            NoTransform => "no-transform",
            OnlyIfCached => "only-if-cached",

            MaxAge(secs) => return write!(f, "max-age={}", secs),
            MaxStale(secs) => return write!(f, "max-stale={}", secs),
            MinFresh(secs) => return write!(f, "min-fresh={}", secs),

            MustRevalidate => "must-revalidate",
            Public => "public",
            Private => "private",
            ProxyRevalidate => "proxy-revalidate",
            SMaxAge(secs) => return write!(f, "s-maxage={}", secs),

            Extension(ref name, None) => &name[..],
            Extension(ref name, Some(ref arg)) => return write!(f, "{}={}", name, arg),

        }, f)
    }
}

impl FromStr for CacheDirective {
    type Err = Option<<u32 as FromStr>::Err>;
    fn from_str(s: &str) -> Result<CacheDirective, Option<<u32 as FromStr>::Err>> {
        use self::CacheDirective::*;
        match s {
            "no-cache" => Ok(NoCache),
            "no-store" => Ok(NoStore),
            "no-transform" => Ok(NoTransform),
            "only-if-cached" => Ok(OnlyIfCached),
            "must-revalidate" => Ok(MustRevalidate),
            "public" => Ok(Public),
            "private" => Ok(Private),
            "proxy-revalidate" => Ok(ProxyRevalidate),
            "" => Err(None),
            _ => match s.find('=') {
                Some(idx) if idx+1 < s.len() => match (&s[..idx], &s[idx+1..].trim_matches('"')) {
                    ("max-age" , secs) => secs.parse().map(MaxAge).map_err(|x| Some(x)),
                    ("max-stale", secs) => secs.parse().map(MaxStale).map_err(|x| Some(x)),
                    ("min-fresh", secs) => secs.parse().map(MinFresh).map_err(|x| Some(x)),
                    ("s-maxage", secs) => secs.parse().map(SMaxAge).map_err(|x| Some(x)),
                    (left, right) => Ok(Extension(left.to_string(), Some(right.to_string())))
                },
                Some(_) => Err(None),
                None => Ok(Extension(s.to_string(), None))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use header::Header;
    use super::*;

    #[test]
    fn test_parse_multiple_headers() {
        let cache = Header::parse_header(&[b"no-cache".to_vec(), b"private".to_vec()]);
        assert_eq!(cache, Some(CacheControl(vec![CacheDirective::NoCache,
                                                 CacheDirective::Private])))
    }

    #[test]
    fn test_parse_argument() {
        let cache = Header::parse_header(&[b"max-age=100, private".to_vec()]);
        assert_eq!(cache, Some(CacheControl(vec![CacheDirective::MaxAge(100),
                                                 CacheDirective::Private])))
    }

    #[test]
    fn test_parse_quote_form() {
        let cache = Header::parse_header(&[b"max-age=\"200\"".to_vec()]);
        assert_eq!(cache, Some(CacheControl(vec![CacheDirective::MaxAge(200)])))
    }

    #[test]
    fn test_parse_extension() {
        let cache = Header::parse_header(&[b"foo, bar=baz".to_vec()]);
        assert_eq!(cache, Some(CacheControl(vec![CacheDirective::Extension("foo".to_string(), None),
                                                 CacheDirective::Extension("bar".to_string(), Some("baz".to_string()))])))
    }

    #[test]
    fn test_parse_bad_syntax() {
        let cache: Option<CacheControl> = Header::parse_header(&[b"foo=".to_vec()]);
        assert_eq!(cache, None)
    }
}

bench_header!(normal, CacheControl, { vec![b"no-cache, private".to_vec(), b"max-age=100".to_vec()] });
