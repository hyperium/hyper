use std::fmt;

#[allow(unused_imports)]
use std::ascii::AsciiExt;

use header::{Header, HeaderFormat, parsing};

/// `Referrer-Policy` header, part of
/// [Referrer Policy](https://www.w3.org/TR/referrer-policy/#referrer-policy-header)
///
/// The `Referrer-Policy` HTTP header specifies the referrer
/// policy that the user agent applies when determining what
/// referrer information should be included with requests made,
/// and with browsing contexts created from the context of the
/// protected resource.
///
/// # ABNF
/// ```plain
/// Referrer-Policy: 1#policy-token
/// policy-token   = "no-referrer" / "no-referrer-when-downgrade"
///                  / "same-origin" / "origin"
///                  / "origin-when-cross-origin" / "unsafe-url"
/// ```
///
/// # Example values
/// * `no-referrer`
///
/// # Example
/// ```
/// use hyper::header::{Headers, ReferrerPolicy};
///
/// let mut headers = Headers::new();
/// headers.set(ReferrerPolicy::NoReferrer);
/// ```
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ReferrerPolicy {
    /// `no-referrer`
    NoReferrer,
    /// `no-referrer-when-downgrade`
    NoReferrerWhenDowngrade,
    /// `same-origin`
    SameOrigin,
    /// `origin`
    Origin,
    /// `origin-when-cross-origin`
    OriginWhenCrossOrigin,
    /// `unsafe-url`
    UnsafeUrl,
     /// `strict-origin`
    StrictOrigin,
    ///`strict-origin-when-cross-origin`
    StrictOriginWhenCrossOrigin,
}

impl Header for ReferrerPolicy {
    fn header_name() -> &'static str {
        static NAME: &'static str = "Referrer-Policy";
        NAME
    }

    fn parse_header(raw: &[Vec<u8>]) -> ::Result<ReferrerPolicy> {
        use self::ReferrerPolicy::*;
        // See https://www.w3.org/TR/referrer-policy/#determine-policy-for-token
        let headers: Vec<String> = try!(parsing::from_comma_delimited(raw));

        for h in headers.iter().rev() {
            let slice = &h.to_ascii_lowercase()[..];
            match slice {
                "no-referrer" | "never" => return Ok(NoReferrer),
                "no-referrer-when-downgrade" | "default" => return Ok(NoReferrerWhenDowngrade),
                "same-origin" => return Ok(SameOrigin),
                "origin" => return Ok(Origin),
                "origin-when-cross-origin" => return Ok(OriginWhenCrossOrigin),
                "strict-origin" => return Ok(StrictOrigin),
                "strict-origin-when-cross-origin" => return Ok(StrictOriginWhenCrossOrigin),
                "unsafe-url" | "always" => return Ok(UnsafeUrl),
                _ => continue,
            }
        }

        Err(::Error::Header)
    }
}

impl HeaderFormat for ReferrerPolicy {
    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for ReferrerPolicy {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::ReferrerPolicy::*;
        f.write_str(match *self {
            NoReferrer => "no-referrer",
            NoReferrerWhenDowngrade => "no-referrer-when-downgrade",
            SameOrigin => "same-origin",
            Origin => "origin",
            OriginWhenCrossOrigin => "origin-when-cross-origin",
            StrictOrigin => "strict-origin",
            StrictOriginWhenCrossOrigin => "strict-origin-when-cross-origin",
            UnsafeUrl => "unsafe-url",
        })
    }
}

#[test]
fn test_parse_header() {
    let a: ReferrerPolicy = Header::parse_header([b"origin".to_vec()].as_ref()).unwrap();
    let b = ReferrerPolicy::Origin;
    assert_eq!(a, b);
    let e: ::Result<ReferrerPolicy> = Header::parse_header([b"foobar".to_vec()].as_ref());
    assert!(e.is_err());
}

#[test]
fn test_rightmost_header() {
    let a: ReferrerPolicy = Header::parse_header(&["same-origin, origin, foobar".into()]).unwrap();
    let b = ReferrerPolicy::Origin;
    assert_eq!(a, b);
}
