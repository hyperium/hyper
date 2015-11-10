// # References
//
// "The Content-Disposition Header Field" https://www.ietf.org/rfc/rfc2183.txt
// "The Content-Disposition Header Field in the Hypertext Transfer Protocol (HTTP)" https://www.ietf.org/rfc/rfc6266.txt
// "Returning Values from Forms: multipart/form-data" https://www.ietf.org/rfc/rfc2388.txt
// Browser conformance tests at: http://greenbytes.de/tech/tc2231/
// IANA assignment: http://www.iana.org/assignments/cont-disp/cont-disp.xhtml

use std::ascii::AsciiExt;
use std::fmt;

use header::{Header, HeaderFormat, parsing};

#[derive(Clone, Debug, PartialEq)]
pub enum DispositionType {
    Inline,
    Attachment,
    Ext(String)
}

#[derive(Clone, Debug, PartialEq)]
pub enum DispositionParam {
    Filename(String),
    Ext(String,String)
}

/// A `Content-Disposition` header, (re)defined in [RFC6266](https://tools.ietf.org/html/rfc6266)
///
/// The Content-Disposition response header field is used to convey
/// additional information about how to process the response payload, and
/// also can be used to attach additional metadata, such as the filename
/// to use when saving the response payload locally.
///
/// # ABNF
/// ```plain
/// content-disposition = "Content-Disposition" ":"
///                       disposition-type *( ";" disposition-parm )
///
/// disposition-type    = "inline" | "attachment" | disp-ext-type
///                       ; case-insensitive
///
/// disp-ext-type       = token
///
/// disposition-parm    = filename-parm | disp-ext-parm
///
/// filename-parm       = "filename" "=" value
///                     | "filename*" "=" ext-value
///
/// disp-ext-parm       = token "=" value
///                     | ext-token "=" ext-value
///
/// ext-token           = <the characters in token, followed by "*">
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct ContentDisposition {
    pub disposition: DispositionType,
    pub parameters: Vec<DispositionParam>,
}

impl Header for ContentDisposition {
    fn header_name() -> &'static str {
        "Content-Disposition"
    }

    fn parse_header(raw: &[Vec<u8>]) -> ::Result<ContentDisposition> {
        parsing::from_one_raw_str(raw).and_then(|s: String| {
            let mut sections = s.split(';');
            let disposition = match sections.next() {
                Some(s) => s.trim().to_ascii_lowercase(),
                None => return Err(::Error::Header),
            };

            let mut cd = ContentDisposition {
                disposition: match &*disposition {
                    "inline" => DispositionType::Inline,
                    "attachment" => DispositionType::Attachment,
                    _ => DispositionType::Ext(disposition),
                },
                parameters: Vec::new(),
            };

            for section in sections {
                let mut parts = section.split('=');

                let key = if let Some(key) = parts.next() {
                    key.trim().to_ascii_lowercase()
                } else {
                    return Err(::Error::Header);
                };

                let mut val = if let Some(val) = parts.next() {
                    val.trim()
                } else {
                    return Err(::Error::Header);
                };

                if val.chars().next() == Some('"') && val.chars().rev().next() == Some('"') {
                    // Unwrap the quotation marks.
                    val = &val[1..val.len() - 1];
                }

                cd.parameters.push(
                    match &*key {
                        "filename" => DispositionParam::Filename(val.to_owned()),
                        _ => DispositionParam::Ext(key, val.to_owned()),
                    }
                );
            }

            Ok(cd)
        })
    }
}


impl HeaderFormat for ContentDisposition {
    #[inline]
    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self, f)
    }
}

impl fmt::Display for ContentDisposition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.disposition {
            DispositionType::Inline => try!(write!(f, "inline")),
            DispositionType::Attachment => try!(write!(f, "attachment")),
            DispositionType::Ext(ref s) => try!(write!(f, "{}", s)),
        }
        for param in self.parameters.iter() {
            match param {
                &DispositionParam::Filename(ref v) => try!(write!(f, "; filename=\"{}\"", v)),
                &DispositionParam::Ext(ref k, ref v) => try!(write!(f, "; {}=\"{}\"", k, v)),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{ContentDisposition,DispositionType,DispositionParam};
    use ::header::Header;

    #[test]
    fn parse_header() {
        assert!(ContentDisposition::parse_header([b"".to_vec()].as_ref()).is_err());

        let a = [b"form-data; dummy=3; name=upload;\r\n filename=\"sample.png\"".to_vec()];
        let a: ContentDisposition = ContentDisposition::parse_header(a.as_ref()).unwrap();
        let b = ContentDisposition {
            disposition: DispositionType::Ext("form-data".to_owned()),
            parameters: vec![ DispositionParam::Ext("dummy".to_owned(), "3".to_owned()),
                              DispositionParam::Ext("name".to_owned(), "upload".to_owned()),
                              DispositionParam::Filename("sample.png".to_owned()) ]
        };
        assert_eq!(a, b);

        let a = [b"attachment; filename=\"image.jpg\"".to_vec()];
        let a: ContentDisposition = ContentDisposition::parse_header(a.as_ref()).unwrap();
        let b = ContentDisposition {
            disposition: DispositionType::Attachment,
            parameters: vec![ DispositionParam::Filename("image.jpg".to_owned()) ]
        };
        assert_eq!(a, b);

    }
}
