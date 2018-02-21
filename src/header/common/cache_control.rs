use std::fmt;
use std::str::FromStr;
use header::{Header, Raw};
use header::parsing::{from_comma_delimited, fmt_comma_delimited};

/// `Cache-Control` header, defined in [RFC7234](https://tools.ietf.org/html/rfc7234#section-5.2)
///
/// The `Cache-Control` header field is used to specify directives for
/// caches along the request/response chain.  Such cache directives are
/// unidirectional in that the presence of a directive in a request does
/// not imply that the same directive is to be given in the response.
///
/// # ABNF
///
/// ```text
/// Cache-Control   = 1#cache-directive
/// cache-directive = token [ "=" ( token / quoted-string ) ]
/// ```
///
/// # Example values
///
/// * `no-cache`
/// * `private, community="UCI"`
/// * `max-age=30`
///
/// # Examples
/// ```
/// use hyper::header::{Headers, CacheControl, CacheDirective};
///
/// let mut headers = Headers::new();
/// headers.set(
///     CacheControl(vec![CacheDirective::MaxAge(86400u32)])
/// );
/// ```
///
/// ```
/// use hyper::header::{Headers, CacheControl, CacheDirective, CacheDirectiveExtension};
///
/// let mut headers = Headers::new();
/// headers.set(
///     CacheControl(vec![
///         CacheDirective::NoCache,
///         CacheDirective::Private,
///         CacheDirective::MaxAge(360_u32),
///         CacheDirective::Extension(CacheDirectiveExtension::StaleWhileRevalidate(360_u32)),
///     ])
/// );
/// ```
#[derive(PartialEq, Clone, Debug)]
pub struct CacheControl(pub Vec<CacheDirective>);

__hyper__deref!(CacheControl => Vec<CacheDirective>);

//TODO: this could just be the header! macro
impl Header for CacheControl {
    fn header_name() -> &'static str {
        static NAME: &'static str = "Cache-Control";
        NAME
    }

    fn parse_header(raw: &Raw) -> ::Result<CacheControl> {
        let directives = try!(from_comma_delimited(raw));
        if !directives.is_empty() {
            Ok(CacheControl(directives))
        } else {
            Err(::Error::Header)
        }
    }

    fn fmt_header(&self, f: &mut ::header::Formatter) -> fmt::Result {
        f.fmt_line(self)
    }
}

impl fmt::Display for CacheControl {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt_comma_delimited(f, &self[..])
    }
}

/// Cache-Control directive extensions.
#[derive(PartialEq, Copy, Clone, Debug)]
pub enum CacheDirectiveExtension {
    /// The [`immutable`](http://tools.ietf.org/html/8246) Cache-Control Extension
    Immutable,
    /// The [`stale-while-revalidate`](http://tools.ietf.org/html/5861) Cache-Control Extension
    StaleWhileRevalidate(u32),
    /// The [`stale-if-error`](http://tools.ietf.org/html/5861) Cache-Control Extension
    StaleIfError(u32),
}

impl fmt::Display for CacheDirectiveExtension {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::CacheDirectiveExtension::*;
        fmt::Display::fmt(match *self {
            Immutable => "immutable",
            StaleWhileRevalidate(secs) => return write!(f, "stale-while-revalidate={}", secs),
            StaleIfError(secs) => return write!(f, "stale-if-error={}", secs),
        }, f)
    }
}

/// `CacheControl` contains a list of these directives.
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

    /// Extension directives.
    Extension(CacheDirectiveExtension),
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

            Extension(ref extension) => return fmt::Display::fmt(extension, f),
        }, f)
    }
}

impl FromStr for CacheDirective {
    type Err = Option<<u32 as FromStr>::Err>;
    fn from_str(s: &str) -> Result<CacheDirective, Option<<u32 as FromStr>::Err>> {
        use self::CacheDirective::*;
        use self::CacheDirectiveExtension::*;
        match s {
            "no-cache" => Ok(NoCache),
            "no-store" => Ok(NoStore),
            "no-transform" => Ok(NoTransform),
            "only-if-cached" => Ok(OnlyIfCached),
            "must-revalidate" => Ok(MustRevalidate),
            "public" => Ok(Public),
            "private" => Ok(Private),
            "proxy-revalidate" => Ok(ProxyRevalidate),
            "immutable" => Ok(Extension(Immutable)),
            "" => Err(None),
            _ => {
                let mut parts_it = s.splitn(2, '=');
                let (lhs, rhs) = (parts_it.next().ok_or(None)?, parts_it.next().ok_or(None)?);
                // FIXME: Use proper quoted-string parsing for rhs.
                let secs: u32 = rhs.trim_matches('"').parse().map_err(Some)?;
                Ok(match lhs {
                    "max-age" => MaxAge(secs),
                    "max-stale" => MaxStale(secs),
                    "min-fresh" => MinFresh(secs),
                    "s-maxage" => SMaxAge(secs),
                    "stale-while-revalidate" => Extension(StaleWhileRevalidate(secs)),
                    "stale-if-error" => Extension(StaleIfError(secs)),
                    _ => Err(None)?
                })
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
        let cache = Header::parse_header(&vec![b"no-cache".to_vec(), b"private".to_vec()].into());
        assert_eq!(cache.ok(), Some(CacheControl(vec![CacheDirective::NoCache,
                                                 CacheDirective::Private])))
    }

    #[test]
    fn test_parse_argument() {
        let cache = Header::parse_header(&vec![b"max-age=100, private".to_vec()].into());
        assert_eq!(cache.ok(), Some(CacheControl(vec![CacheDirective::MaxAge(100),
                                                 CacheDirective::Private])))
    }

    #[test]
    fn test_parse_quote_form() {
        let cache = Header::parse_header(&vec![b"max-age=\"200\"".to_vec()].into());
        assert_eq!(cache.ok(), Some(CacheControl(vec![CacheDirective::MaxAge(200)])))
    }

    #[test]
    fn test_parse_extension() {
        let cache = Header::parse_header(&vec![b"immutable, stale-if-error=100".to_vec()].into());
        let extensions = vec![CacheDirective::Extension(CacheDirectiveExtension::Immutable),
              CacheDirective::Extension(CacheDirectiveExtension::StaleIfError(100))];
        assert_eq!(cache.ok(), Some(CacheControl(extensions)))
    }

    #[test]
    fn test_parse_bad_syntax() {
        let cache: ::Result<CacheControl> = Header::parse_header(&vec![b"foo=".to_vec()].into());
        assert_eq!(cache.ok(), None)
    }
}

bench_header!(normal,
    CacheControl, { vec![b"no-cache, private".to_vec(), b"max-age=100".to_vec()] });
