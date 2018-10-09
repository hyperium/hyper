use std::cell::RefCell;
use std::fmt::{self, Write};
use std::str;

use http::header::HeaderValue;
use time::{self, Duration};

// "Sun, 06 Nov 1994 08:49:37 GMT".len()
pub const DATE_VALUE_LENGTH: usize = 29;

pub fn extend(dst: &mut Vec<u8>) {
    CACHED.with(|cache| {
        dst.extend_from_slice(cache.borrow().buffer());
    })
}

pub fn update() {
    CACHED.with(|cache| {
        cache.borrow_mut().check();
    })
}

pub(crate) fn update_and_header_value() -> HeaderValue {
    CACHED.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache.check();
        HeaderValue::from_bytes(cache.buffer())
            .expect("Date format should be valid HeaderValue")
    })
}

struct CachedDate {
    bytes: [u8; DATE_VALUE_LENGTH],
    pos: usize,
    next_update: time::Timespec,
}

thread_local!(static CACHED: RefCell<CachedDate> = RefCell::new(CachedDate::new()));

impl CachedDate {
    fn new() -> Self {
        let mut cache = CachedDate {
            bytes: [0; DATE_VALUE_LENGTH],
            pos: 0,
            next_update: time::Timespec::new(0, 0),
        };
        cache.update(time::get_time());
        cache
    }

    fn buffer(&self) -> &[u8] {
        &self.bytes[..]
    }

    fn check(&mut self) {
        let now = time::get_time();
        if now > self.next_update {
            self.update(now);
        }
    }

    fn update(&mut self, now: time::Timespec) {
        self.pos = 0;
        let _ = write!(self, "{}", time::at_utc(now).rfc822());
        debug_assert!(self.pos == DATE_VALUE_LENGTH);
        self.next_update = now + Duration::seconds(1);
        self.next_update.nsec = 0;
    }
}

impl fmt::Write for CachedDate {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let len = s.len();
        self.bytes[self.pos..self.pos + len].copy_from_slice(s.as_bytes());
        self.pos += len;
        Ok(())
    }
}

#[test]
fn test_date_len() {
    assert_eq!(DATE_VALUE_LENGTH, "Sun, 06 Nov 1994 08:49:37 GMT".len());
}
