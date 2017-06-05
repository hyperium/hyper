use std::fmt;
use std::mem;
use header::{Header, HeaderFormat};
use std::collections::HashMap;
use unicase::UniCase;
use error::Result;
use header::CowStr;
use std::borrow::Cow;

#[derive(Debug, Clone)]
pub struct WwwAuthenticate(HashMap<UniCase<CowStr>, Vec<RawChallenge>>);

pub trait Challenge: Clone {
    fn challenge_name() -> &'static str;
    fn from_raw(raw: RawChallenge) -> Option<Self>;
    fn into_raw(self) -> RawChallenge;
}


impl WwwAuthenticate {
    pub fn new() -> Self {
        WwwAuthenticate(HashMap::new())
    }

    pub fn get<C: Challenge>(&self) -> Option<Vec<C>> {
        self.0
            .get(&UniCase(CowStr(Cow::Borrowed(C::challenge_name()))))
            .map(|m| m.iter().map(Clone::clone).flat_map(C::from_raw).collect())
    }

    pub fn get_raw(&self, name: &str) -> Option<&[RawChallenge]> {
        self.0
            .get(&UniCase(CowStr(Cow::Borrowed(unsafe {
                                                  mem::transmute::<&str, &'static str>(name)
                                              }))))
            .map(AsRef::as_ref)
    }

    pub fn set<C: Challenge>(&mut self, c: C) -> bool {
        self.0
            .insert(UniCase(CowStr(Cow::Borrowed(C::challenge_name()))),
                    vec![c.into_raw()])
            .is_some()
    }

    pub fn set_raw(&mut self, scheme: String, raw: RawChallenge) -> bool {
        self.0
            .insert(UniCase(CowStr(Cow::Owned(scheme))), vec![raw])
            .is_some()
    }

    pub fn append<C: Challenge>(&mut self, c: C) {
        self.0
            .entry(UniCase(CowStr(Cow::Borrowed(C::challenge_name()))))
            .or_insert(Vec::new())
            .push(c.into_raw())
    }

    pub fn append_raw(&mut self, scheme: String, raw: RawChallenge) {
        self.0
            .entry(UniCase(CowStr(Cow::Owned(scheme))))
            .or_insert(Vec::new())
            .push(raw)
    }

    pub fn has<C: Challenge>(&self) -> bool {
        self.get::<C>().is_some()
    }
}


impl Header for WwwAuthenticate {
    fn header_name() -> &'static str {
        "WWW-Authenticate"
    }
    fn parse_header(raw: &[Vec<u8>]) -> Result<Self> {
        let mut map = HashMap::new();
        for data in raw {
            let stream = parser::Stream::new(data.as_ref());
            loop {
                let (scheme, challenge) = match stream.challenge() {
                    Ok(v) => v,
                    Err(e) => {
                        if stream.is_end() {
                            break;
                        } else {
                            return Err(e);
                        }
                    }
                };
                // TODO: treat the cases when a scheme is duplicated
                map.entry(UniCase(CowStr(Cow::Owned(scheme))))
                    .or_insert(Vec::new())
                    .push(challenge);
            }

        }
        Ok(WwwAuthenticate(map))
    }
}

impl HeaderFormat for WwwAuthenticate {
    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (scheme, values) in &self.0 {
            for value in values.iter() {
                // tail commas are allowed
                write!(f, "{} {}, ", scheme, value)?;
            }
        }
        Ok(())
    }
}

#[test]
fn test_www_authenticate_multiple_headers() {
    let input1 = br#"Digest realm="http-auth@example.org", qop="auth, auth-int", algorithm=SHA-256, nonce="7ypf/xlj9XXwfDPEoM4URrv/xwf94BcCAzFZH4GiTo0v", opaque="FQhe/qaU925kfnzjCev0ciny7QMkPqMAFRtzCUYo5tdS""#.to_vec();
    let input2 = br#"Digest realm="http-auth@example.org", qop="auth, auth-int", algorithm=MD5, nonce="7ypf/xlj9XXwfDPEoM4URrv/xwf94BcCAzFZH4GiTo0v", opaque="FQhe/qaU925kfnzjCev0ciny7QMkPqMAFRtzCUYo5tdS""#.to_vec();
    let input = &[input1, input2];

    let auth = WwwAuthenticate::parse_header(input).unwrap();
    let digests = auth.get::<DigestChallenge>().unwrap();
    assert!(digests.contains(&DigestChallenge {
                                 realm: Some("http-auth@example.org".into()),
                                 qop: Some(vec![Qop::Auth, Qop::AuthInt]),
                                 algorithm: Some(Algorithm::Sha256),
                                 nonce: Some("7ypf/xlj9XXwfDPEoM4URrv/xwf94BcCAzFZH4GiTo0v".into()),
                                 opaque: Some("FQhe/qaU925kfnzjCev0ciny7QMkPqMAFRtzCUYo5tdS"
                                                  .into()),
                                 domain: None,
                                 stale: None,
                                 userhash: None,
                             }));

    assert!(digests.contains(&DigestChallenge {
                                 realm: Some("http-auth@example.org".into()),
                                 qop: Some(vec![Qop::Auth, Qop::AuthInt]),
                                 algorithm: Some(Algorithm::Md5),
                                 nonce: Some("7ypf/xlj9XXwfDPEoM4URrv/xwf94BcCAzFZH4GiTo0v".into()),
                                 opaque: Some("FQhe/qaU925kfnzjCev0ciny7QMkPqMAFRtzCUYo5tdS"
                                                  .into()),
                                 domain: None,
                                 stale: None,
                                 userhash: None,
                             }));


}

macro_rules! try_opt {
    ($e: expr) => {
        match $e {
            Some(e) => e,
            None => return None
        }
    }
}


pub use self::raw::*;
mod raw {
    use super::*;
    use unicase::UniCase;
    use header::CowStr;
    use std::borrow::Cow;
    use std::mem;

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Quote {
        Always,
        IfNeed,
    }

    #[derive(Debug, Clone)]
    pub struct ChallengeFields(HashMap<UniCase<CowStr>, (String, Quote)>);

    impl ChallengeFields {
        pub fn new() -> Self {
            ChallengeFields(HashMap::new())
        }
        // fn values(&self) -> Values<K, V>
        // fn values_mut(&mut self) -> ValuesMut<K, V>
        // fn iter(&self) -> Iter<K, V>
        // fn iter_mut(&mut self) -> IterMut<K, V>
        // fn entry(&mut self, key: K) -> Entry<K, V>
        pub fn len(&self) -> usize {
            self.0.len()
        }
        pub fn is_empty(&self) -> bool {
            self.0.is_empty()
        }
        // fn drain(&mut self) -> Drain<K, V>
        pub fn clear(&mut self) {
            self.0.clear()
        }
        pub fn get(&self, k: &str) -> Option<&String> {
            self.0
                .get(&UniCase(CowStr(Cow::Borrowed(unsafe {
                                                      mem::transmute::<&str, &'static str>(k)
                                                  }))))
                .map(|&(ref s, _)| s)
        }
        pub fn contains_key(&self, k: &str) -> bool {
            self.0
                .contains_key(&UniCase(CowStr(Cow::Borrowed(unsafe {
                    mem::transmute::<&str, &'static str>(k)
                }))))
        }
        pub fn get_mut(&mut self, k: &str) -> Option<&mut String> {
            self.0
                .get_mut(&UniCase(CowStr(Cow::Borrowed(unsafe {
                                                          mem::transmute::<&str, &'static str>(k)
                                                      }))))
                .map(|&mut (ref mut s, _)| s)
        }
        pub fn insert(&mut self, k: String, v: String) -> Option<String> {
            self.0
                .insert(UniCase(CowStr(Cow::Owned(k))), (v, Quote::IfNeed))
                .map(|(s, _)| s)
        }
        pub fn insert_quoting(&mut self, k: String, v: String) -> Option<String> {
            self.0
                .insert(UniCase(CowStr(Cow::Owned(k))), (v, Quote::Always))
                .map(|(s, _)| s)
        }
        pub fn insert_static(&mut self, k: &'static str, v: String) -> Option<String> {
            self.0
                .insert(UniCase(CowStr(Cow::Borrowed(k))), (v, Quote::IfNeed))
                .map(|(s, _)| s)
        }
        pub fn insert_static_quoting(&mut self, k: &'static str, v: String) -> Option<String> {
            self.0
                .insert(UniCase(CowStr(Cow::Borrowed(k))), (v, Quote::Always))
                .map(|(s, _)| s)
        }
        pub fn remove(&mut self, k: &str) -> Option<String> {
            self.0
                .remove(&UniCase(CowStr(Cow::Borrowed(unsafe {
                                                         mem::transmute::<&str, &'static str>(k)
                                                     }))))
                .map(|(s, _)| s)
        }
    }
    // index

    #[derive(Debug, Clone)]
    pub enum RawChallenge {
        Token68(String),
        Fields(ChallengeFields),
    }

    fn need_quote(s: &str, q: &Quote) -> bool {
        if q == &Quote::Always {
            true
        } else {
            s.bytes().any(|c| !parser::is_token_char(c))
        }
    }

    impl fmt::Display for RawChallenge {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            use self::RawChallenge::*;
            match *self {
                Token68(ref token) => write!(f, "{}", token)?,
                Fields(ref fields) => {
                    for (k, &(ref v, ref quote)) in fields.0.iter() {
                        if need_quote(v, quote) {
                            write!(f, "{}={:?}, ", k, v)?
                        } else {
                            write!(f, "{}={}, ", k, v)?
                        }
                    }
                }
            }
            Ok(())
        }
    }
}

pub use self::basic::*;
mod basic {
    use super::*;
    use super::raw::RawChallenge;

    #[derive(Debug, Clone, Eq, PartialEq, Hash)]
    pub struct BasicChallenge {
        pub realm: String,
        // pub charset: Option<Charset>
    }

    impl Challenge for BasicChallenge {
        fn challenge_name() -> &'static str {
            "Basic"
        }
        fn from_raw(raw: RawChallenge) -> Option<Self> {
            use self::RawChallenge::*;
            match raw {
                Token68(_) => return None,
                Fields(mut map) => {
                    let realm = try_opt!(map.remove("realm"));
                    // only "UTF-8" is allowed.
                    // See https://tools.ietf.org/html/rfc7617#section-2.1
                    match map.remove("charset") {
                        Some(c) => {
                            if UniCase(&c) == UniCase("UTF-8") {
                                ()
                            } else {
                                return None;
                            }
                        }
                        None => (),
                    }
                    if !map.is_empty() {
                        return None;
                    }
                    Some(BasicChallenge { realm: realm })
                }
            }
        }
        fn into_raw(self) -> RawChallenge {
            let mut map = ChallengeFields::new();
            map.insert_static("realm", self.realm);
            RawChallenge::Fields(map)
        }
    }

    #[test]
    fn test_parse_basic() {
        let input = b"Basic realm=\"secret zone\"".to_vec();
        let auth = WwwAuthenticate::parse_header(&[input]).unwrap();
        let mut basics = auth.get::<BasicChallenge>().unwrap();
        assert_eq!(basics.len(), 1);
        let basic = basics.swap_remove(0);
        assert_eq!(basic.realm, "secret zone")
    }

    #[test]
    fn test_roundtrip_basic() {
        let basic = BasicChallenge { realm: "secret zone".into() };
        let mut auth = WwwAuthenticate::new();
        auth.set(basic.clone());
        let data = format!("{}", &auth as &(HeaderFormat + Send + Sync));
        let auth = WwwAuthenticate::parse_header(&[data.into_bytes()]).unwrap();
        let basic_tripped = auth.get::<BasicChallenge>().unwrap().swap_remove(0);
        assert_eq!(basic, basic_tripped);
    }
}

pub use self::digest::*;
mod digest {
    use super::*;
    use url::Url;
    use std::str::FromStr;

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct DigestChallenge {
        pub realm: Option<String>,
        pub domain: Option<Vec<Url>>,
        pub nonce: Option<String>,
        pub opaque: Option<String>,
        pub stale: Option<bool>,
        pub algorithm: Option<Algorithm>,
        pub qop: Option<Vec<Qop>>,
        // pub charset: Option<Charset>,
        pub userhash: Option<bool>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub enum Algorithm {
        Md5,
        Md5Sess,
        Sha512Trunc256,
        Sha512Trunc256Sess,
        Sha256,
        Sha256Sess,
        Other(String),
    }


    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub enum Qop {
        Auth,
        AuthInt,
    }


    impl Challenge for DigestChallenge {
        fn challenge_name() -> &'static str {
            "Digest"
        }
        fn from_raw(raw: RawChallenge) -> Option<Self> {
            use self::RawChallenge::*;
            match raw {
                Token68(_) => return None,
                Fields(mut map) => {
                    let realm = map.remove("realm");
                    let domains = map.remove("domain");
                    let nonce = map.remove("nonce");
                    let opaque = map.remove("opaque");
                    let stale = map.remove("stale");
                    let algorithm = map.remove("algorithm");
                    let qop = map.remove("qop");
                    let charset = map.remove("charset");
                    let userhash = map.remove("userhash");

                    if !map.is_empty() {
                        return None;
                    }

                    let domains = domains.and_then(|ds| {
                                                       ds.split_whitespace()
                                                           .map(Url::from_str)
                                                           .map(::std::result::Result::ok)
                                                           .collect::<Option<Vec<Url>>>()
                                                   });
                    let stale = stale.map(|s| s == "true");
                    let algorithm = algorithm.map(|a| {
                        use self::Algorithm::*;
                        match a.as_str() {
                            "MD5" => return Md5,
                            "MD5-sess" => return Md5Sess,
                            "SHA-512-256" => return Sha512Trunc256,
                            "SHA-512-256-sess" => return Sha512Trunc256Sess,
                            "SHA-256" => return Sha256,
                            "SHA-256-sess" => return Sha256Sess,
                            _ => (),
                        };
                        return Other(a);
                    });
                    let qop = match qop {
                        None => None,
                        Some(qop) => {
                            let mut v = vec![];
                            let s = parser::Stream::new(qop.as_bytes());
                            loop {
                                match try_opt!(s.token().ok()) {
                                    "auth" => v.push(Qop::Auth),
                                    "auth-int" => v.push(Qop::AuthInt),
                                    _ => (),
                                }
                                try_opt!(s.skip_field_sep().ok());
                                if s.is_end() {
                                    break;
                                }
                            }
                            Some(v)
                        }
                    };
                    match charset {
                        Some(c) => {
                            if UniCase(&c) == UniCase("UTF-8") {
                                ()
                            } else {
                                return None;
                            }
                        }
                        None => (),
                    }

                    let userhash = userhash.and_then(|u| match u.as_str() {
                                                         "true" => Some(true),
                                                         "false" => Some(false),
                                                         _ => None,
                                                     });
                    Some(DigestChallenge {
                             realm: realm,
                             domain: domains,
                             nonce: nonce,
                             opaque: opaque,
                             stale: stale,
                             algorithm: algorithm,
                             qop: qop,
                             // pub charset: Option<Charset>,
                             userhash: userhash,
                         })

                }
            }
        }
        fn into_raw(self) -> RawChallenge {
            let mut map = ChallengeFields::new();
            // Notes on quoting/non-quoting from the spec  ttps://tools.ietf.org/html/rfc7616#section-3.3
            //
            // > For historical reasons, a sender MUST only generate the quoted string
            // > syntax values for the following parameters: realm, domain, nonce,
            // > opaque, and qop.
            // >
            // > For historical reasons, a sender MUST NOT generate the quoted string
            // > syntax values for the following parameters: stale and algorithm.

            for realm in self.realm {
                map.insert_static_quoting("realm", realm);
            }

            for domain in self.domain {
                let mut d = String::new();
                d.extend(domain.into_iter().map(Url::into_string).map(|s| s + " "));
                let len = d.len();
                d.truncate(len - 1);
                map.insert_static_quoting("domain", d);

            }
            for nonce in self.nonce {
                map.insert_static_quoting("nonce", nonce);
            }
            for opaque in self.opaque {
                map.insert_static_quoting("opaque", opaque);
            }
            for stale in self.stale {
                map.insert_static("stale", format!("{}", stale));
            }
            for algorithm in self.algorithm {
                map.insert_static("algorithm", format!("{}", algorithm));
            }
            for qop in self.qop {
                let mut q = String::new();
                q.extend(qop.into_iter().map(|q| format!("{}", q)).map(|s| s + ", "));
                let len = q.len();
                q.truncate(len - 2);
                map.insert_static_quoting("qop", q);
            }
            for userhash in self.userhash {
                map.insert_static("userhash", format!("{}", userhash));
            }
            RawChallenge::Fields(map)
        }
    }

    impl fmt::Display for Algorithm {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            use self::Algorithm::*;
            match *self {
                Md5 => write!(f, "MD5"),
                Md5Sess => write!(f, "MD5-sess"),
                Sha512Trunc256 => write!(f, "SHA-512-256"),
                Sha512Trunc256Sess => write!(f, "SHA-512-256-sess"),
                Sha256 => write!(f, "SHA-256"),
                Sha256Sess => write!(f, "SHA-256-sess"),
                Other(ref s) => write!(f, "{}", s),
            }
        }
    }


    impl fmt::Display for Qop {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            use self::Qop::*;
            match *self {
                Auth => write!(f, "auth"),
                AuthInt => write!(f, "auth-int"),
            }
        }
    }




    #[test]
    fn test_parse_digest() {
        let input = br#"Digest realm="http-auth@example.org", qop="auth, auth-int", algorithm=SHA-256, nonce="7ypf/xlj9XXwfDPEoM4URrv/xwf94BcCAzFZH4GiTo0v", opaque="FQhe/qaU925kfnzjCev0ciny7QMkPqMAFRtzCUYo5tdS""#;
        let auth = WwwAuthenticate::parse_header(&[input.to_vec()]).unwrap();
        let mut digests = auth.get::<DigestChallenge>().unwrap();
        assert_eq!(digests.len(), 1);
        let digest = digests.swap_remove(0);
        assert_eq!(digest.realm, Some("http-auth@example.org".into()));
        assert_eq!(digest.domain, None);
        assert_eq!(digest.nonce,
                   Some("7ypf/xlj9XXwfDPEoM4URrv/xwf94BcCAzFZH4GiTo0v".into()));
        assert_eq!(digest.opaque,
                   Some("FQhe/qaU925kfnzjCev0ciny7QMkPqMAFRtzCUYo5tdS".into()));
        assert_eq!(digest.stale, None);
        assert_eq!(digest.algorithm, Some(Algorithm::Sha256));
        assert_eq!(digest.qop, Some(vec![Qop::Auth, Qop::AuthInt]));
        assert_eq!(digest.userhash, None);
    }

    #[test]
    fn test_roundtrip_digest() {
        let digest = DigestChallenge {
            realm: Some("http-auth@example.org".into()),
            domain: None,
            nonce: Some("7ypf/xlj9XXwfDPEoM4URrv/xwf94BcCAzFZH4GiTo0v".into()),
            opaque: Some("FQhe/qaU925kfnzjCev0ciny7QMkPqMAFRtzCUYo5tdS".into()),
            stale: None,
            algorithm: Some(Algorithm::Sha256),
            qop: Some(vec![Qop::Auth, Qop::AuthInt]),
            userhash: None,
        };
        let mut auth = WwwAuthenticate::new();
        auth.set(digest.clone());
        let data = format!("{}", &auth as &(HeaderFormat + Send + Sync));
        let auth = WwwAuthenticate::parse_header(&[data.into_bytes()]).unwrap();
        let digest_tripped = auth.get::<DigestChallenge>().unwrap().swap_remove(0);
        assert_eq!(digest, digest_tripped);
    }
}


mod parser {
    use error::*;
    use std::str::from_utf8_unchecked;
    use std::cell::Cell;
    use super::raw::{RawChallenge, ChallengeFields};

    pub struct Stream<'a>(Cell<usize>, &'a [u8]);

    pub fn is_ws(c: u8) -> bool {
        // See https://tools.ietf.org/html/rfc7230#section-3.2.3
        b"\t ".contains(&c)
    }

    pub fn is_token_char(c: u8) -> bool {
        // See https://tools.ietf.org/html/rfc7230#section-3.2.6
        br#"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890!#$%&'*+-.^_`|~"#
            .contains(&c)
    }

    impl<'a> Stream<'a> {
        pub fn new(data: &'a [u8]) -> Self {
            Stream(Cell::from(0), data)
        }

        pub fn inc(&self, i: usize) {
            let pos = self.pos();
            self.0.set(pos + i);
        }

        pub fn pos(&self) -> usize {
            self.0.get()
        }

        pub fn is_end(&self) -> bool {
            self.1.len() <= self.pos()
        }

        pub fn cur(&self) -> u8 {
            self.1[self.pos()]
        }

        pub fn skip_a(&self, c: u8) -> Result<()> {
            if self.cur() == c {
                self.inc(1);
                Ok(())
            } else {
                Err(Error::Header)
            }
        }
        pub fn skip_a_next(&self, c: u8) -> Result<()> {
            self.skip_ws()?;
            if self.is_end() {
                return Err(Error::Header);
            }
            self.skip_a(c)
        }

        pub fn take_while<F>(&self, f: F) -> Result<&[u8]>
            where F: Fn(u8) -> bool
        {
            let start = self.pos();
            while !self.is_end() && f(self.cur()) {
                self.inc(1);
            }
            Ok(&self.1[start..self.pos()])
        }

        pub fn take_while1<F>(&self, f: F) -> Result<&[u8]>
            where F: Fn(u8) -> bool
        {
            self.take_while(f)
                .and_then(|b| if b.len() < 1 {
                              Err(Error::Header)
                          } else {
                              Ok(b)
                          })

        }

        pub fn try<F, T>(&self, f: F) -> Result<T>
            where F: FnOnce() -> Result<T>
        {
            let init = self.pos();
            match f() {
                ok @ Ok(_) => ok,
                err @ Err(_) => {
                    self.0.set(init);
                    err
                }
            }
        }


        pub fn skip_ws(&self) -> Result<()> {
            self.take_while(is_ws).map(|_| ())
        }

        pub fn skip_next_comma(&self) -> Result<()> {
            self.skip_a_next(b',')
        }

        pub fn skip_field_sep(&self) -> Result<()> {
            self.skip_ws()?;
            if self.is_end() {
                return Ok(());
            }
            self.skip_next_comma()?;
            while self.skip_next_comma().is_ok() {}
            self.skip_ws()?;
            Ok(())
        }

        pub fn token(&self) -> Result<&str> {
            self.take_while1(is_token_char)
                .map(|s| unsafe { from_utf8_unchecked(s) })
        }

        pub fn next_token(&self) -> Result<&str> {
            self.skip_ws()?;
            self.token()
        }

        pub fn quoted_string(&self) -> Result<String> {
            if self.is_end() {
                return Err(Error::Header);
            }

            if self.cur() != b'"' {
                return Err(Error::Header);
            }
            self.inc(1);
            let mut s = String::new();
            while !self.is_end() && self.cur() != b'"' {
                if self.cur() == b'\\' {
                    self.inc(1)

                }
                s.push(self.cur() as char);
                self.inc(1);
            }
            if self.is_end() {
                return Err(Error::Header);
            } else {
                debug_assert!(self.cur() == b'"');
                self.inc(1);
            }
            Ok(s)
        }


        pub fn token68(&self) -> Result<&str> {
            let start = self.pos();
            // See https://tools.ietf.org/html/rfc7235#section-2.1
            self.take_while1(|c| {
                    b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890-._~+/"
                        .contains(&c)
                })?;
            self.take_while(|c| c == b'=')?;
            Ok(unsafe { from_utf8_unchecked(&self.1[start..self.pos()]) })
        }

        pub fn kv_token(&self) -> Result<(&str, &str)> {
            let k = self.token()?;
            self.skip_a_next(b'=')?;
            self.skip_ws()?;
            let v = self.token()?;
            Ok((k, v))
        }

        pub fn kv_quoted(&self) -> Result<(&str, String)> {
            let k = self.token()?;
            self.skip_a_next(b'=')?;
            self.skip_ws()?;
            let v = self.quoted_string()?;
            Ok((k, v))
        }

        pub fn field(&self) -> Result<(String, String)> {
            self.try(|| self.kv_token().map(|(k, v)| (k.to_string(), v.to_string())))
                .or_else(|_| self.kv_quoted().map(|(k, v)| (k.to_string(), v)))
        }

        pub fn raw_token68(&self) -> Result<RawChallenge> {
            let ret = self.token68()
                .map(ToString::to_string)
                .map(RawChallenge::Token68)?;
            self.skip_field_sep()?;
            Ok(ret)
        }

        pub fn raw_fields(&self) -> Result<RawChallenge> {
            let mut map = ChallengeFields::new();
            loop {
                match self.try(|| self.field()) {
                    Err(_) => return Ok(RawChallenge::Fields(map)),
                    Ok((k, v)) => {
                        if self.skip_field_sep().is_ok() {
                            if map.insert(k, v).is_some() {
                                // field key must not be duplicated
                                return Err(Error::Header);
                            }
                            if self.is_end() {
                                return Ok(RawChallenge::Fields(map));
                            }
                        } else {
                            return Err(Error::Header);
                        }
                    }
                }
            }
        }

        pub fn challenge(&self) -> Result<(String, RawChallenge)> {
            let scheme = self.next_token()?;
            self.take_while1(is_ws)?;
            let challenge = self.try(|| self.raw_token68())
                .or_else(|_| self.raw_fields())?;
            Ok((scheme.to_string(), challenge))
        }
    }

    #[test]
    fn test_parese_quoted_field() {
        let b = b"realm=\"secret zone\"";
        let stream = Stream::new(b);
        let (k, v) = stream.field().unwrap();
        assert_eq!(k, "realm");
        assert_eq!(v, "secret zone");
        assert!(stream.is_end());
    }

    #[test]
    fn test_parese_token_field() {
        let b = b"algorithm=MD5";
        let stream = Stream::new(b);
        let (k, v) = stream.field().unwrap();
        assert_eq!(k, "algorithm");
        assert_eq!(v, "MD5");
        assert!(stream.is_end());
    }

    #[test]
    fn test_parese_raw_quoted_fields() {
        let b = b"realm=\"secret zone\"";
        let stream = Stream::new(b);
        match stream.raw_fields().unwrap() {
            RawChallenge::Token68(_) => panic!(),
            RawChallenge::Fields(fields) => {
                assert_eq!(fields.len(), 1);
                assert_eq!(fields.get("realm").unwrap(), "secret zone");
            }
        }
        assert!(stream.is_end());
    }

    #[test]
    fn test_parese_raw_token_fields() {
        let b = b"algorithm=MD5";
        let stream = Stream::new(b);
        match stream.raw_fields().unwrap() {
            RawChallenge::Token68(_) => panic!(),
            RawChallenge::Fields(fields) => {
                assert_eq!(fields.len(), 1);
                assert_eq!(fields.get("algorithm").unwrap(), "MD5");
            }
        }
        assert!(stream.is_end());
    }
    #[test]
    fn test_parese_token68() {
        let b = b"auea1./+=";
        let stream = Stream::new(b);
        let token = stream.token68().unwrap();
        assert_eq!(token, "auea1./+=");
        assert!(stream.is_end());
    }

    #[test]
    fn test_parese_raw_token68() {
        let b = b"auea1./+=";
        let stream = Stream::new(b);
        match stream.raw_token68().unwrap() {
            RawChallenge::Token68(token) => assert_eq!(token, "auea1./+="),
            RawChallenge::Fields(_) => panic!(),
        }
        assert!(stream.is_end());
    }

    #[test]
    fn test_parese_challenge1() {
        let b = b"Token abceaqj13-.+=";
        let stream = Stream::new(b);
        match stream.challenge().unwrap() {
            (scheme, RawChallenge::Token68(token)) => {
                assert_eq!(scheme, "Token");
                assert_eq!(token, "abceaqj13-.+=");
            }
            (_, RawChallenge::Fields(_)) => panic!(),
        }
        assert!(stream.is_end());
    }

    #[test]
    fn test_parese_challenge2() {
        let b = b"Basic realm=\"secret zone\"";
        let stream = Stream::new(b);
        match stream.challenge().unwrap() {
            (_, RawChallenge::Token68(_)) => panic!(),
            (scheme, RawChallenge::Fields(fields)) => {
                assert_eq!(scheme, "Basic");
                assert_eq!(fields.len(), 1);
                assert_eq!(fields.get("realm").unwrap(), "secret zone");
            }
        }
        assert!(stream.is_end());
    }

    #[test]
    fn test_parese_challenge3() {
        let b = b"Bearer token=aeub8_";
        let stream = Stream::new(b);
        match stream.challenge().unwrap() {
            (_, RawChallenge::Token68(_)) => panic!(),
            (scheme, RawChallenge::Fields(fields)) => {
                assert_eq!(scheme, "Bearer");
                assert_eq!(fields.len(), 1);
                assert_eq!(fields.get("token").unwrap(), "aeub8_");
            }
        }
        assert!(stream.is_end());
    }

    #[test]
    fn test_parese_challenge4() {
        let b = b"Bearer token=aeub8_, user=\"fooo\"";
        let stream = Stream::new(b);
        match stream.challenge().unwrap() {
            (_, RawChallenge::Token68(_)) => panic!(),
            (scheme, RawChallenge::Fields(fields)) => {
                assert_eq!(scheme, "Bearer");
                assert_eq!(fields.len(), 2);
                assert_eq!(fields.get("token").unwrap(), "aeub8_");
                assert_eq!(fields.get("user").unwrap(), "fooo");
            }
        }
        assert!(stream.is_end());
    }

    #[test]
    fn test_parese_challenge5() {
        let b = b"Bearer user=\"fooo\",,, token=aeub8_,,";
        let stream = Stream::new(b);
        match stream.challenge().unwrap() {
            (_, RawChallenge::Token68(_)) => panic!(),
            (scheme, RawChallenge::Fields(fields)) => {
                assert_eq!(scheme, "Bearer");
                assert_eq!(fields.len(), 2);
                assert_eq!(fields.get("token").unwrap(), "aeub8_");
                assert_eq!(fields.get("user").unwrap(), "fooo");
            }
        }
        assert!(stream.is_end());
    }



    #[test]
    #[should_panic]
    fn test_parse_null() {
        let b = b"";
        let stream = Stream::new(b);
        println!("{:?}", stream.challenge().unwrap());
    }
}
