// # References
//
// "The Content-Disposition Header Field" https://www.ietf.org/rfc/rfc2183.txt
// "The Content-Disposition Header Field in the Hypertext Transfer Protocol (HTTP)" https://www.ietf.org/rfc/rfc6266.txt
// "Returning Values from Forms: multipart/form-data" https://www.ietf.org/rfc/rfc2388.txt
// Browser conformance tests at: http://greenbytes.de/tech/tc2231/
// IANA assignment: http://www.iana.org/assignments/cont-disp/cont-disp.xhtml

use std::ascii::AsciiExt;
use std::fmt;
use std::str::FromStr;

use header::{Header, HeaderFormat, parsing};
use header::shared::Charset;

/// The implied disposition of the content of the HTTP body
#[derive(Clone, Debug, PartialEq)]
pub enum DispositionType {
    /// Inline implies default processing
    Inline,
    /// Attachment implies that the recipient should prompt the user to save the response locally,
    /// rather than process it normally (as per its media type).
    Attachment,
    /// Extension type.  Should be handled by recipients the same way as Attachment
    Ext(String)
}

/// A parameter to the disposition type
#[derive(Clone, Debug, PartialEq)]
pub enum DispositionParam {
    /// A Filename consisting of a Charset, an optional Language-tag string as defined by RFC 5646
    /// section 2.1, and finally a sequence of bytes representing the filename
    Filename(Charset, Option<String>, Vec<u8>),
    /// Extension type consisting of token and value.  Recipients should ignore unrecognized
    /// parameters.
    Ext(String, String)
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
    /// The disposition
    pub disposition: DispositionType,
    /// Disposition parameters
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

                let val = if let Some(val) = parts.next() {
                    val.trim()
                } else {
                    return Err(::Error::Header);
                };

                cd.parameters.push(
                    match &*key {
                        "filename" => DispositionParam::Filename(
                            Charset::Ext("UTF-8".to_owned()), None, unquote_value(val).into_bytes()),
                        "filename*" => {
                            let (charset, opt_language, value) = try!(parse_ext_value(val));
                            DispositionParam::Filename(charset, opt_language, value)
                        },
                        _ => DispositionParam::Ext(key, unquote_value(val)),
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
                &DispositionParam::Filename(ref charset, ref opt_lang, ref bytes) => {
                    let mut use_simple_format: bool = false;
                    if opt_lang.is_none() {
                        if let Charset::Ext(ref ext) = *charset {
                            if ext.to_ascii_lowercase() == "utf-8" {
                                use_simple_format = true;
                            }
                        }
                    }
                    if use_simple_format {
                        try!(write!(f, "; filename=\"{}\"",
                                    match String::from_utf8(bytes.clone()) {
                                        Ok(s) => s,
                                        Err(_) => return Err(fmt::Error),
                                    }));
                    } else {
                        let langstr = match *opt_lang {
                            Some(ref lang) => lang.clone(),
                            None => "".to_owned(),
                        };
                        try!(write!(f, "; filename*={}'{}'", charset, langstr));
                        try!(f.write_str(&*try!(bytes_to_value_chars(bytes))))
                    }
                },
                &DispositionParam::Ext(ref k, ref v) => try!(write!(f, "; {}=\"{}\"", k, v)),
            }
        }
        Ok(())
    }
}

fn unquote_value(val: &str) -> String {
    if val.chars().next() == Some('"') && val.chars().rev().next() == Some('"') {
        // Unwrap the quotation marks.
        (&val[1..val.len() - 1]).to_owned()
    } else {
        val.to_owned()
    }
}

fn parse_ext_value(val: &str) -> ::Result<(Charset, Option<String>, Vec<u8>)> {
    // https://tools.ietf.org/html/rfc5987#section-3.2
    // ext-value     = charset  "'" [ language ] "'" value-chars
    //               ; like RFC 2231's <extended-initial-value>
    //               ; (see [RFC2231], Section 7)
    //
    // charset       = "UTF-8" / "ISO-8859-1" / mime-charset
    //
    // mime-charset  = 1*mime-charsetc
    // mime-charsetc = ALPHA / DIGIT
    //               / "!" / "#" / "$" / "%" / "&"
    //               / "+" / "-" / "^" / "_" / "`"
    //               / "{" / "}" / "~"
    //               ; as <mime-charset> in Section 2.3 of [RFC2978]
    //               ; except that the single quote is not included
    //               ; SHOULD be registered in the IANA charset registry
    //
    // language      = <Language-Tag, defined in [RFC5646], Section 2.1>
    //
    // value-chars   = *( pct-encoded / attr-char )
    //
    // pct-encoded   = "%" HEXDIG HEXDIG
    //               ; see [RFC3986], Section 2.1
    //
    // attr-char     = ALPHA / DIGIT
    //               / "!" / "#" / "$" / "&" / "+" / "-" / "."
    //               / "^" / "_" / "`" / "|" / "~"
    //               ; token except ( "*" / "'" / "%" )

    // Break into three pieces separated by the single-quote character
    let parts: Vec<&str> = val.split('\'').collect();
    if parts.len() != 3 {
        return Err(::Error::Header);
    }

    // Interpret the first piece as a Charset
    let charset: Charset = try!(FromStr::from_str(parts[0]));

    // Interpret the second piece as a language tag
    // FIXME, we currently take any string for this
    let lang: Option<String> = match parts[1] {
        "" => None,
        s => Some(s.to_owned()),
    };

    // Interpret the third piece as a sequence of value characters
    let value: Vec<u8> = try!(value_chars_to_bytes(parts[2]));

    Ok( (charset, lang, value) )
}

fn value_chars_to_bytes(value_chars: &str) -> ::Result<Vec<u8>> {
    let mut output: Vec<u8> = Vec::new();

    let mut iter = value_chars.chars();
    loop {
        match iter.next() {
            None => return Ok(output),
            Some('%') => {
                let mut byte: u8 = 0;
                match iter.next() {
                    None => return Err(::Error::Header), // hex char expected,
                    Some(c) => byte += 16 * try!(c.to_digit(16).ok_or(::Error::Header)) as u8,
                }
                match iter.next() {
                    None => return Err(::Error::Header), // hex char expected,
                    Some(c) => byte += try!(c.to_digit(16).ok_or(::Error::Header)) as u8,
                }
                output.push(byte);
            },
            Some(other) => match other {
                'a'...'z' | 'A'...'Z' | '0'...'9' | '!' | '#' | '$' | '&' |
                '+' | '-' | '.' | '^' | '_' | '`' | '|' | '~' => output.push(other as u8),
                _ => return Err(::Error::Header) // invalid value character
            }
        }
    }
}

fn bytes_to_value_chars(bytes: &Vec<u8>) -> Result<String,fmt::Error> {
    let mut output: String = String::new();
    let mut buffer: [u8; 8] = [0; 8];
    for byte in bytes.iter() {
        match *byte as char {
            'a'...'z' | 'A'...'Z' | '0'...'9' | '!' | '#' | '$' | '&' |
            '+' | '-' | '.' | '^' | '_' | '`' | '|' | '~' => output.push(*byte as char),
            other => {
                let count = other.encode_utf8(&mut buffer).unwrap();
                for i in 0..count {
                    output.push('%');
                    let high: u8 = buffer[i] / 16;
                    match high {
                        0...9 => output.push((b'0' + high) as char),
                        10...15 => output.push((b'A' + high) as char),
                        _ => panic!("Not reachable bytes_to_value_chars()"),
                    }
                    let low: u8 = buffer[i] % 16;
                    match low {
                        0...9 => output.push((b'0' + low) as char),
                        10...15 => output.push((b'A' + low) as char),
                        _ => panic!("Not reachable bytes_to_value_chars()"),
                    }
                }
            }
        }
    }
    Ok(String::new()) // FIXME GINA
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
// e.g. title*=iso-8859-1'en'%A3%20rates
//   or title*=UTF-8''%c2%a3%20and%20%e2%82%ac%20rates

