use std::cell::RefCell;
use std::fmt::{self, Write};
use std::str;

use time::{self, Duration};

// "Sun, 06 Nov 1994 08:49:37 GMT".len()
pub const DATE_VALUE_LENGTH: usize = 29;

pub(crate) fn extend(dst: &mut Vec<u8>) {
    CACHED.with(|cache| {
        let mut cache = cache.borrow_mut();
        if !cache.interval {
            cache.update_without_interval();
        }
        dst.extend_from_slice(cache.bytes());
    })
}

pub(crate) fn update_interval() {
    CACHED.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache.interval = true;
        cache.render(time::get_time());
    })
}

pub(crate) fn interval_off() {
    CACHED.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache.interval = false;
    })
}

struct CachedDate {
    bytes: [u8; DATE_VALUE_LENGTH],
    pos: usize,
    next_update: time::Timespec,
    interval: bool,
}

thread_local!(static CACHED: RefCell<CachedDate> = RefCell::new(CachedDate {
    bytes: [0; DATE_VALUE_LENGTH],
    pos: 0,
    next_update: time::Timespec::new(0, 0),
    interval: false,
}));

impl CachedDate {
    fn bytes(&self) -> &[u8] {
        &self.bytes[..]
    }

    fn render(&mut self, now: time::Timespec) {
        self.pos = 0;
        write!(self, "{}", time::at_utc(now).rfc822()).unwrap();
        debug_assert!(self.pos == DATE_VALUE_LENGTH);
    }

    fn update_without_interval(&mut self) {
        let now = time::get_time();
        if now > self.next_update {
            self.render(now);
            self.next_update = now + Duration::seconds(1);
            self.next_update.nsec = 0;
        }
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
