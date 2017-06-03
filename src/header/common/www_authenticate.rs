use std::fmt;
// use std::ops::{Deref, DerefMut};
// use base64::{encode, decode};
use header::{Header, HeaderFormat};
use std::collections::HashMap;
use unicase::UniCase;
use error::{Error, Result};
use header::CowStr;
use std::borrow::Cow;
//use header::internals::cell::PtrMapCell;

#[derive(Debug, Clone)]
pub struct WwwAuthenticate(HashMap<UniCase<CowStr>, RawChallenge>);

pub trait Challenge: Clone {
    fn challenge_name() -> &'static str;
    fn from_raw(raw: RawChallenge) -> Option<Self>;
    fn to_raw(self) -> RawChallenge;
}


impl WwwAuthenticate {
    pub fn new() -> Self {
        WwwAuthenticate(HashMap::new())
    }

    pub fn get<C: Challenge>(&self) -> Option<C> {
        self.0
            .get(&UniCase(CowStr(Cow::Borrowed(C::challenge_name()))))
            .map(Clone::clone)
            .and_then(C::from_raw)
    }

    pub fn set<C: Challenge>(&mut self, c: C) {
        self.0.clear();
        self.add(c);
    }

    pub fn add<C: Challenge>(&mut self, c: C) -> bool {
        self.0
            .insert(UniCase(CowStr(Cow::Borrowed(C::challenge_name()))),
                    c.to_raw())
            .is_some()
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
        if raw.len() != 1 {
            return Err(Error::Header);
        }
        let data = &raw[0];
        let stream = parser::Stream::new(data.as_ref());
        let mut map = HashMap::new();
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
            map.insert(UniCase(CowStr(Cow::Owned(scheme))), challenge);
        }
        Ok(WwwAuthenticate(map))
    }
}

impl HeaderFormat for WwwAuthenticate {
    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "WWW-Authenticate: ")?;
        for (k, v) in &self.0 {
            // tail commas are allowed
            write!(f, "{} {}, ", k, v)?;
        }
        Ok(())
    }
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

    #[derive(Debug, Clone)]
    pub struct ChallengeFields(HashMap<UniCase<CowStr>, String>);
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
        }
        pub fn insert(&mut self, k: String, v: String) -> Option<String> {
            self.0.insert(UniCase(CowStr(Cow::Owned(k))), v)
        }
        pub fn insert_static(&mut self, k: &'static str, v: String) -> Option<String> {
            self.0.insert(UniCase(CowStr(Cow::Borrowed(k))), v)
        }
        pub fn remove(&mut self, k: &str) -> Option<String> {
            self.0
                .remove(&UniCase(CowStr(Cow::Borrowed(unsafe {
                                                         mem::transmute::<&str, &'static str>(k)
                                                     }))))
        }
    }
    // index

    #[derive(Debug, Clone)]
    pub enum RawChallenge {
        Token68(String),
        Fields(ChallengeFields),
    }

    fn need_quote(_: &str) -> bool {
        true
    }

    impl fmt::Display for RawChallenge {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            use self::RawChallenge::*;
            match *self {
                Token68(ref token) => write!(f, "{}", token)?,
                Fields(ref fields) => {
                    for (ref k, ref v) in fields.0.iter() {
                        if need_quote(v) {
                            write!(f, "{}={:?}", k, v)?
                        } else {
                            write!(f, "{}={}", k, v)?
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

    #[derive(Debug, Clone)]
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
        fn to_raw(self) -> RawChallenge {
            let mut map = ChallengeFields::new();
            map.insert_static("realm", self.realm);
            RawChallenge::Fields(map)
        }
    }

    #[test]
    fn test_parse_basic() {
        let input = b"Basic realm=\"secret zone\"".to_vec();
        let auth = WwwAuthenticate::parse_header(&[input]).unwrap();
        let basic = auth.get::<BasicChallenge>().unwrap();
        assert_eq!(basic.realm, "secret zone")
    }

    #[test]
    fn test_format_basic() {
        let mut auth = WwwAuthenticate::new();
        auth.add(BasicChallenge { realm: "secret zone".into() });
        let auth = format!("{}", &auth as &(HeaderFormat + Send + Sync));
        assert_eq!(auth, "WWW-Authenticate: Basic realm=\"secret zone\", ")
    }

}

pub use self::digest::*;
mod digest {
    //use super::*;
    // #[derive(Debug, Clone)]
    // struct DigestChallenge {
    //     realm: String,
    //     domain: Option<Uri>,
    //     nonce: String,
    //     opaque: Option<String>,
    //     stale: Option<bool>,
    //     algolithm: Option<Algorithm>,
    //     qop: Option<Qop>,
    // }

    // enum Algorithim {
    //     Md5,
    //     Md5Sess,
    //     Other(String),
    // }


    // enum Qop {
    //     Auth,
    //     AuthInit,
    //     Other(String)
    // }
}


mod parser {
    use error::*;
    use std::str::from_utf8_unchecked;
    use std::cell::Cell;
    use super::raw::{RawChallenge, ChallengeFields};

    pub struct Stream<'a>(Cell<usize>, &'a [u8]);

    fn is_ws(c: u8) -> bool {
        // See https://tools.ietf.org/html/rfc7230#section-3.2.3
        b"\t ".contains(&c)
    }

    fn is_token_char(c: u8) -> bool {
        // See https://tools.ietf.org/html/rfc7230#section-3.2.6
        b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890!#$%&'*+-.^_`|~"
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
            let mut s = String::new();
            if !self.1[0] == b'"' {
                return Err(Error::Header);
            }
            self.inc(1);
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
            self.skip_ws()?;
            let k = self.token()?;
            self.skip_a_next(b'=')?;
            self.skip_ws()?;
            let v = self.token()?;
            Ok((k, v))
        }

        pub fn kv_quoted(&self) -> Result<(&str, String)> {
            self.skip_ws()?;
            let k = self.token()?;
            self.skip_a_next(b'=')?;
            self.skip_ws()?;
            let v = self.quoted_string()?;
            Ok((k, v))
        }

        pub fn field(&self) -> Result<(String, String)> {
            self.try(|| self.kv_quoted())
                .map(|(k, v)| (k.to_string(), v))
                .or_else(|_| self.kv_token().map(|(k, v)| (k.to_string(), v.to_string())))
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
        let b = b"Bearer user=\"fooo\", token=aeub8_";
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
