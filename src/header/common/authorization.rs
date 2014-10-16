extern crate serialize;

use header::Header;
use std::fmt::{mod, Show};
use self::serialize::base64::{ToBase64, FromBase64, Standard, Config};
use super::util::from_one_raw_str;
use std::from_str::FromStr;

/// The `Authorization` header field.
#[deriving(Clone, PartialEq, Show)]
pub struct Authorization(pub Credentials);

/// Different types of credentials that we know how to handle.  If we
/// don't know how to parse it, we'll use a Raw string. On the
/// flipside, if you want to create an Authorization header without a
/// corresponding type, you can use Raw as well.
#[deriving(Clone, PartialEq, Show)]
pub enum Credentials {
    /// An unknown credential type
    Raw(String),
    /// HTTP Basic Authentication with username and password. We
    /// handle the base64 encode/decode.
    Basic(BasicCredentials)
}

/// Credential holder for Basic Authentication
#[deriving(Clone, PartialEq, Show)]
pub struct BasicCredentials {
    /// The username as a possibly empty string
    pub username: String,

    /// The password. `None` if the `:` delimiter character was not
    /// part of the parsed input.
    pub password: Option<String>
}

impl FromStr for Authorization {
    fn from_str(s: &str) -> Option<Authorization> {
        debug!("Authorization::from_str =? {}", s);
        let mut auth_parts = s.split(' ');
        let auth_scheme = auth_parts.next();

        let result = match auth_scheme {
            Some("Basic") => {
                let basic_credentials = auth_parts.next();
                match basic_credentials {
                    Some(basic_credentials) =>
                        match basic_credentials.from_base64() {
                            Ok(user_pass) =>
                                match String::from_utf8(user_pass) {
                                    Ok(user_pass) => {
                                        let user_pass_parts: Vec<&str> = user_pass.as_slice().split(':').collect();
                                        let username = user_pass_parts[0].to_string();
                                        let password = match user_pass_parts.len() {
                                            2 => Some(user_pass_parts[1].to_string()),
                                            _ => None
                                        };
                                        Some(Some(Authorization(Basic( BasicCredentials {username: username, password:password} ))))
                                    },
                                    Err(_) =>
                                        None
                                },
                            Err(_) =>
                                None
                        },
                    _ =>
                        None
                }
            },
            _ => None
        };
        
        // fall back if we weren't able to fully parse the scheme
        match result {
            Some(result) => result,
            None => Some(Authorization(Raw(s.to_string())))
        }
    }
}

impl Header for Authorization {
    fn header_name(_: Option<Authorization>) -> &'static str {
        "Authorization"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<Authorization> {
        from_one_raw_str(raw)
    }

    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let Authorization(ref credential_type) = *self;
        match *credential_type {
            Raw(ref value) =>
                value.fmt(fmt),
            Basic(ref credentials) =>
                format!("Basic {}", 
                        match credentials.password {
                            Some(ref password) =>
                                format!("{}:{}", credentials.username, password),
                            None =>
                                format!("{}:", credentials.username)
                        }.as_bytes().to_base64(Config {char_set: Standard, pad: true, line_length: None})
                        ).fmt(fmt)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::MemReader;
    use super::{Authorization, Raw, Basic, BasicCredentials};
    use super::super::super::{Headers};

    fn mem(s: &str) -> MemReader {
        MemReader::new(s.as_bytes().to_vec())
    }

    #[test]
    fn test_raw_auth() {
        let mut headers = Headers::new();
        headers.set(Authorization(Raw("foo bar baz".to_string())));
        assert_eq!(headers.to_string(), "Authorization: foo bar baz\r\n".to_string());
    }

    #[test]
    fn test_raw_auth_parse() {
        let headers = Headers::from_raw(&mut mem("Authorization: foo bar baz\r\n\r\n")).unwrap();
        let Authorization(ref authtype) = *headers.get::<Authorization>().unwrap();
        match *authtype {
            Raw(ref credentials) =>
                assert_eq!(credentials.as_slice(), "foo bar baz"),
            _ => 
                fail!()
        }
    }

    #[test]
    fn test_basic_auth() {
        let mut headers = Headers::new();
        headers.set(Authorization(Basic(BasicCredentials { username: "Aladdin".to_string(), password: Some("open sesame".to_string()) })));
        assert_eq!(headers.to_string(), "Authorization: Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ==\r\n".to_string());
    }

    #[test]
    fn test_basic_auth_no_password() {
        let mut headers = Headers::new();
        headers.set(Authorization(Basic(BasicCredentials { username: "Aladdin".to_string(), password: None })));
        assert_eq!(headers.to_string(), "Authorization: Basic QWxhZGRpbjo=\r\n".to_string());
    }

    #[test]
    fn test_basic_auth_parse() {
        let headers = Headers::from_raw(&mut mem("Authorization: Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ==\r\n\r\n")).unwrap();
        let Authorization(ref authtype) = *headers.get::<Authorization>().unwrap();
        match *authtype {
            Basic(ref credentials) => {
                assert_eq!(credentials.username.as_slice(), "Aladdin");
                assert_eq!(credentials.password, Some("open sesame".to_string()));
            }
            _ => 
                fail!()
        }
    }

    #[test]
    fn test_basic_auth_parse_no_password() {
        let headers = Headers::from_raw(&mut mem("Authorization: Basic QWxhZGRpbjo=\r\n\r\n")).unwrap();
        let Authorization(ref authtype) = *headers.get::<Authorization>().unwrap();
        match *authtype {
            Basic(ref credentials) => {
                assert_eq!(credentials.username.as_slice(), "Aladdin");
                assert_eq!(credentials.password, Some("".to_string()));
            }
            _ => 
                fail!()
        }
    }

}
