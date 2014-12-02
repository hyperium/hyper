use std::fmt;
use std::str::FromStr;
use header::{Header, HeaderFormat};
use super::util::{from_one_comma_delimited, fmt_comma_delimited};

/// The Cache-Control header.
#[deriving(PartialEq, Clone, Show)]
pub struct CacheControl(pub Vec<CacheDirective>);

deref!(CacheControl -> Vec<CacheDirective>)

impl Header for CacheControl {
    fn header_name(_: Option<CacheControl>) -> &'static str {
        "Cache-Control"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<CacheControl> {
        let directives = raw.iter()
            .filter_map(|line| from_one_comma_delimited(line[]))
            .collect::<Vec<Vec<CacheDirective>>>()
            .concat_vec();
        if directives.len() > 0 {
            Some(CacheControl(directives))
        } else {
            None
        }
    }
}

impl HeaderFormat for CacheControl {
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt_comma_delimited(fmt, self[])
    }
}

/// CacheControl contains a list of these directives.
#[deriving(PartialEq, Clone)]
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
    MaxAge(uint),
    /// "max-stale=delta"
    MaxStale(uint),
    /// "min-fresh=delta"
    MinFresh(uint),

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
    SMaxAge(uint),

    /// Extension directives. Optionally include an argument.
    Extension(String, Option<String>)
}

impl fmt::Show for CacheDirective {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::CacheDirective::*;
        match *self {
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

            Extension(ref name, None) => name[],
            Extension(ref name, Some(ref arg)) => return write!(f, "{}={}", name, arg),

        }.fmt(f)
    }
}

impl FromStr for CacheDirective {
    fn from_str(s: &str) -> Option<CacheDirective> {
        use self::CacheDirective::*;
        match s {
            "no-cache" => Some(NoCache),
            "no-store" => Some(NoStore),
            "no-transform" => Some(NoTransform),
            "only-if-cached" => Some(OnlyIfCached),
            "must-revalidate" => Some(MustRevalidate),
            "public" => Some(Public),
            "private" => Some(Private),
            "proxy-revalidate" => Some(ProxyRevalidate),
            "" => None,
            _ => match s.find('=') {
                Some(idx) if idx+1 < s.len() => match (s[..idx], s[idx+1..].trim_chars('"')) {
                    ("max-age" , secs) => from_str::<uint>(secs).map(MaxAge),
                    ("max-stale", secs) => from_str::<uint>(secs).map(MaxStale),
                    ("min-fresh", secs) => from_str::<uint>(secs).map(MinFresh),
                    ("s-maxage", secs) => from_str::<uint>(secs).map(SMaxAge),
                    (left, right) => Some(Extension(left.into_string(), Some(right.into_string())))
                },
                Some(_) => None,
                None => Some(Extension(s.into_string(), None))
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

bench_header!(normal, CacheControl, { vec![b"no-cache, private".to_vec(), b"max-age=100".to_vec()] })
