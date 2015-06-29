use header::{Header, HeaderFormat};
use std::fmt::{self};

/// `StrictTransportSecurity` header, defined in [RFC6797](https://tools.ietf.org/html/rfc6797)
///
/// This specification defines a mechanism enabling web sites to declare
/// themselves accessible only via secure connections and/or for users to be
/// able to direct their user agent(s) to interact with given sites only over
/// secure connections.  This overall policy is referred to as HTTP Strict
/// Transport Security (HSTS).  The policy is declared by web sites via the
/// Strict-Transport-Security HTTP response header field and/or by other means,
/// such as user agent configuration, for example.
///
/// # Example values
/// * `max-age=31536000`
/// * `max-age=15768000 ; includeSubDomains`
/// 
/// # Example
/// ```
/// # extern crate hyper;
/// # fn main() {
/// use hyper::header::{Headers, StrictTransportSecurity};
///
/// let mut headers = Headers::new();
///
/// headers.set(
///    StrictTransportSecurity(true, 31536000u64)
/// );
/// # }
/// ```
#[derive(Clone, PartialEq, Debug)]
pub struct StrictTransportSecurity(pub bool, pub u64);

enum StsDirective {
    IncludeSubdomains(bool),
    MaxAge(u64)
}

struct StsParser {
    pos: usize,
    input: Vec<u8>
}

impl StsParser {
    fn peek(&self) -> char {
        *self.input[self.pos..].iter().next().unwrap() as char
    }

    fn eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn pop(&mut self) -> char {
        let c = *self.input[self.pos..].iter().next().unwrap() as char;
        self.pos += 1;
        c
    }

    fn pop_while<F>(&mut self, test: F) -> String where F: Fn(char) -> bool {
        let mut result = String::new();
        while !self.eof() && test(self.peek()) {
            result.push(self.pop());
        }

        result
    }

    fn pop_whitespace(&mut self) {
        self.pop_while(|c: char| c.is_whitespace());
    }

    fn pop_to_sep(&mut self) {
        self.pop_while(|c| c != ';');

        if !self.eof() {
            self.pop();
        }
    }

    fn parse_tokens(&mut self) -> String {
        self.pop_whitespace();

        let result = self.pop_while(|c| match c {
            '(' | ')' | '<' | '>' | '@' | ',' | ';' | ':' | '\\' |
            '"' | '/' | '[' | ']' | '?' | '=' | '{' | '}' | ' ' | '\t' => false,
            _ => true
        });

        self.pop_whitespace();

        result
    }

    fn parse_directive_name(&mut self) -> String {
        self.parse_tokens()
    }

    fn parse_value(&mut self) -> String {
        self.parse_tokens()
    }

    fn parse_quoted_string(&mut self) -> String {
        let mut result = String::new();
        self.pop();

        loop {
            let next = match self.pop() {
                '\\' => self.pop(),
                '"' => break,
                c @ _ => c
            };

            result.push(next)
        }

        self.pop_whitespace();

        result
    }

    fn parse_directive_value(&mut self) -> Option<String> {
        self.pop_whitespace();
        if self.eof() { return None }

        match self.peek() {
            '=' => {
                self.pop();
                self.pop_whitespace();

                if self.eof() { return None }

                let result = match self.peek() {
                    '"' => Some(self.parse_quoted_string()),
                    _ => Some(self.parse_value())
                };

                self.pop_to_sep();

                result
            }
            _ => {
                self.pop_to_sep();

                None
            }
        }
    }

    fn parse_directive(&mut self) -> Option<StsDirective> {
        let directive_name = self.parse_directive_name().to_lowercase();

        match &*directive_name {
            "max-age" => {
                match self.parse_directive_value() {
                    Some(max_age_val) => {
                        match max_age_val.parse::<u64>() {
                            Ok(max_age) => Some(StsDirective::MaxAge(max_age)),
                            _ => None
                        }
                    },
                    None => None
                }
            }
            "includesubdomains" => Some(StsDirective::IncludeSubdomains(true)),
            _ => None
        }
    }

    fn parse(input: Vec<u8>) -> ::Result<StrictTransportSecurity> {
        let mut parser = StsParser { pos: 0, input: input };
        let mut directives = Vec::new();

        while !parser.eof() {
            directives.push(parser.parse_directive());
        }

        let (include_subdomains, max_age) = directives.iter().fold((None, None), |m, d| match d {
            &Some(StsDirective::MaxAge(a)) => (m.0, Some(a)),
            &Some(StsDirective::IncludeSubdomains(i)) => (Some(i), m.1),
            _ => m
        });

        match (include_subdomains, max_age) {
            (Some(_), Some(max_age)) => Ok(StrictTransportSecurity(true, max_age)),
            (None, Some(max_age)) => Ok(StrictTransportSecurity(false, max_age)),
            _ => Err(::Error::Header)
        }
    }
}

impl Header for StrictTransportSecurity {
    fn header_name() -> &'static str {
        "Strict-Transport-Security"
    }

    fn parse_header(raw: &[Vec<u8>]) -> ::Result<StrictTransportSecurity> {
        if let Some(first_header_raw) = raw.iter().nth(0) {
            StsParser::parse(first_header_raw.clone())
        } else {
            Err(::Error::Header)
        }
    }
}

impl HeaderFormat for StrictTransportSecurity {
    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let &StrictTransportSecurity(include_subdomains, max_age) = self;

        if include_subdomains {
            write!(f, "max-age={}; includeSubdomains", max_age)
        } else {
            write!(f, "max-age={}", max_age)
        }
    }
}

#[test]
fn test_parse_max_age() {
    let h = Header::parse_header(&[b"max-age=31536000".to_vec()][..]);
    assert_eq!(h.ok(), Some(StrictTransportSecurity(false, 31536000u64)));
}

#[test]
fn test_parse_max_age_no_value() {
    let h: ::Result<StrictTransportSecurity> = Header::parse_header(&[b"max-age".to_vec()][..]);
    assert!(h.is_err());
}

#[test]
fn test_parse_quoted_max_age() {
    let h = Header::parse_header(&[b"max-age=\"31536000\"".to_vec()][..]);
    assert_eq!(h.ok(), Some(StrictTransportSecurity(false, 31536000u64)));
}

#[test]
fn test_parse_spaces_max_age() {
    let h = Header::parse_header(&[b"max-age = 31536000".to_vec()][..]);
    assert_eq!(h.ok(), Some(StrictTransportSecurity(false, 31536000u64)));
}

#[test]
fn test_parse_include_subdomains() {
    let h = Header::parse_header(&[b"max-age=15768000 ; includeSubDomains".to_vec()][..]);
    assert_eq!(h.ok(), Some(StrictTransportSecurity(true, 15768000u64)));
}

#[test]
fn test_parse_no_max_age() {
    let h: ::Result<StrictTransportSecurity> = Header::parse_header(&[b"includeSubDomains".to_vec()][..]);
    assert!(h.is_err());
}

#[test]
fn test_parse_max_age_nan() {
    let h: ::Result<StrictTransportSecurity> = Header::parse_header(&[b"max-age = derp".to_vec()][..]);
    assert!(h.is_err());
}

bench_header!(bench, StrictTransportSecurity, { vec![b"max-age=15768000 ; includeSubDomains".to_vec()] });
