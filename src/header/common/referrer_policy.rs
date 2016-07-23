use std::fmt;
use std::ascii::AsciiExt;

use header::{Header, Raw, parsing};

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
}

impl Header for ReferrerPolicy {
    fn header_name() -> &'static str {
        static NAME: &'static str = "Referrer-Policy";
        NAME
    }

    fn parse_header(raw: &Raw) -> ::Result<ReferrerPolicy> {
        use self::ReferrerPolicy::*;
        parsing::from_one_raw_str(raw).and_then(|s: String| {
            let slice = &s.to_ascii_lowercase()[..];
            // See https://www.w3.org/TR/referrer-policy/#determine-policy-for-token
            match slice {
                "no-referrer" | "never" => Ok(NoReferrer),
                "no-referrer-when-downgrade" | "default" => Ok(NoReferrerWhenDowngrade),
                "same-origin" => Ok(SameOrigin),
                "origin" => Ok(Origin),
                "origin-when-cross-origin" => Ok(OriginWhenCrossOrigin),
                "unsafe-url" | "always" => Ok(UnsafeUrl),
                _ => Err(::Error::Header),
            }
        })
    }

    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::ReferrerPolicy::*;
        f.write_str(match *self {
            NoReferrer => "no-referrer",
            NoReferrerWhenDowngrade => "no-referrer-when-downgrade",
            SameOrigin => "same-origin",
            Origin => "origin",
            OriginWhenCrossOrigin => "origin-when-cross-origin",
            UnsafeUrl => "unsafe-url",
        })
    }
}

#[test]
fn test_parse_header() {
    let a: ReferrerPolicy = Header::parse_header(&"origin".into()).unwrap();
    let b = ReferrerPolicy::Origin;
    assert_eq!(a, b);
    let e: ::Result<ReferrerPolicy> = Header::parse_header(&"foobar".into());
    assert!(e.is_err());
}
