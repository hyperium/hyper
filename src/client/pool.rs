use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::time::{Duration, Instant};

use http::KeepAlive;

pub struct Pool<T> {
    map: HashMap<String, Vec<(KeepAlive, T)>>,
    timeout: Duration,
}

impl<T: Clone> Pool<T> {
    pub fn new() -> Pool<T> {
        Pool {
            map: HashMap::new(),
            timeout: Duration::default(),
        }
    }

    pub fn checkout(&mut self, key: &str) -> Option<T> {
        let expired = Instant::now() - self.timeout;
        self.map.get_mut(key).and_then(|list| {
            for entry in list {
                match entry.0.started() {
                    Some(started) if started > expired => {
                        entry.0.reset();
                        return Some(entry.1.clone())
                    },
                    Some(..) => {
                        //TODO: throw this entry away, its expired
                    },
                    None => (),
                }
            }
            None
        })
    }

    pub fn put(&mut self, key: String, entry: (KeepAlive, T)) {
        self.map.entry(key)
            .or_insert(Vec::new())
            .push(entry);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;
    use http::KeepAlive;
    use super::Pool;

    #[test]
    fn test_pool_checkout() {
        let mut pool = Pool::new();
        pool.timeout = Duration::from_secs(5);

        let mut keep_alive = KeepAlive::new();
        keep_alive.start();
        pool.put("foo".to_string(), (keep_alive, 41));

        assert_eq!(pool.checkout("foo"), Some(41));
    }

    #[test]
    fn test_pool_checkout_returns_none_if_expired() {
        let mut pool = Pool::new();
        pool.timeout = Duration::from_secs(1);

        let mut keep_alive = KeepAlive::new();
        keep_alive.start();
        pool.put("foo".to_string(), (keep_alive, 41));

        ::std::thread::sleep(pool.timeout);
        assert_eq!(pool.checkout("foo"), None);
    }

    #[test]
    fn test_pool_removes_expired() {

    }

}
