use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fmt;
use std::io;
use std::ops::{Deref, DerefMut, BitAndAssign};
use std::rc::Rc;
use std::time::{Duration, Instant};

use futures::{Future, Async, Poll};

use http::{KeepAlive, KA};

pub struct Pool<T> {
    inner: Rc<RefCell<PoolInner<T>>>,
}

struct PoolInner<T> {
    idle: HashMap<Rc<String>, Vec<Entry<T>>>,
    timeout: Duration,
}

impl<T: Clone> Pool<T> {
    pub fn new(timeout: Duration) -> Pool<T> {
        Pool {
            inner: Rc::new(RefCell::new(PoolInner {
                idle: HashMap::new(),
                timeout: timeout,
            })),
        }
    }

    pub fn checkout(&mut self, key: &str) -> Checkout<T> {
        Checkout {
            pool: self.clone(),
            key: Rc::new(key.to_owned()),
        }
    }

    fn put(&mut self, key: Rc<String>, entry: Entry<T>) {
        trace!("Pool::put {:?}", key);
        self.inner.borrow_mut()
            .idle.entry(key)
            .or_insert(Vec::new())
            .push(entry);
    }

    pub fn pooled(&self, key: Rc<String>, value: T) -> Pooled<T> {
        trace!("Pool::pooled {:?}", key);
        Pooled {
            entry: Entry {
                value: value,
                is_reused: false,
                status: Rc::new(Cell::new(KA::Busy)),
            },
            key: key,
            pool: self.clone(),
        }
    }

    fn reuse(&self, key: Rc<String>, mut entry: Entry<T>) -> Pooled<T> {
        trace!("Pool::reuse {:?}", key);
        entry.is_reused = true;
        entry.status.set(KA::Busy);
        Pooled {
            entry: entry,
            key: key,
            pool: self.clone(),
        }
    }
}

impl<T> Clone for Pool<T> {
    fn clone(&self) -> Pool<T> {
        Pool {
            inner: self.inner.clone(),
        }
    }
}

#[derive(Clone)]
pub struct Pooled<T> {
    entry: Entry<T>,
    key: Rc<String>,
    pool: Pool<T>,
}

impl<T> Deref for Pooled<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.entry.value
    }
}

impl<T> DerefMut for Pooled<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.entry.value
    }
}

impl<T: Clone> KeepAlive for Pooled<T> {
    fn busy(&mut self) {
        self.entry.status.set(KA::Busy);
    }

    fn disable(&mut self) {
        self.entry.status.set(KA::Disabled);
    }

    fn idle(&mut self) {
        self.entry.status.set(KA::Idle(Instant::now()));
        self.entry.is_reused = true;
        self.pool.put(self.key.clone(), self.entry.clone());
    }

    fn status(&self) -> KA {
        self.entry.status.get()
    }
}

impl<T> fmt::Debug for Pooled<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Pooled")
            .field("status", &self.entry.status.get())
            .field("key", &self.key)
            .finish()
    }
}

impl<T: Clone> BitAndAssign<bool> for Pooled<T> {
    fn bitand_assign(&mut self, enabled: bool) {
        if !enabled {
            self.disable();
        }
    }
}

#[derive(Clone)]
struct Entry<T> {
    value: T,
    is_reused: bool,
    status: Rc<Cell<KA>>,
}

pub struct Checkout<T> {
    pool: Pool<T>,
    key: Rc<String>,
}

impl<T: Clone> Future for Checkout<T> {
    type Item = Pooled<T>;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let expired = Instant::now() - self.pool.inner.borrow().timeout;
        let key = &self.key;
        trace!("Pool::checkout url = {:?}, expiration = {:?}", key, expired);
        let mut should_remove = false;
        let entry = self.pool.inner.borrow_mut().idle.get_mut(key).and_then(|list| {
            trace!("Pool::checkout key found {:?}", key);
            while let Some(entry) = list.pop() {
                match entry.status.get() {
                    KA::Idle(idle_at) if idle_at > expired => {
                        trace!("Pool::checkout found idle client for {:?}", key);
                        should_remove = list.is_empty();
                        return Some(entry);
                    },
                    _ => {
                        // every other case the Entry should just be dropped
                        // 1. Idle but expired
                        // 2. Busy (something else somehow took it?)
                        // 3. Disabled don't reuse of course
                    }
                }
            }
            should_remove = true;
            None
        });

        if should_remove {
            self.pool.inner.borrow_mut().idle.remove(key);
        }
        match entry {
            Some(entry) => Ok(Async::Ready(self.pool.reuse(self.key.clone(), entry))),
            None => Ok(Async::NotReady),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use std::time::Duration;
    use futures::{Async, Future};
    use http::KeepAlive;
    use super::Pool;

    #[test]
    fn test_pool_checkout() {
        let mut pool = Pool::new(Duration::from_secs(5));
        let key = Rc::new("foo".to_string());
        let mut pooled = pool.pooled(key.clone(), 41);
        pooled.idle();

        match pool.checkout(&key).poll().unwrap() {
            Async::Ready(pooled) => assert_eq!(*pooled, 41),
            _ => panic!("not ready"),
        }
    }

    #[test]
    fn test_pool_checkout_returns_none_if_expired() {
        let mut pool = Pool::new(Duration::from_secs(1));
        let key = Rc::new("foo".to_string());
        let mut pooled = pool.pooled(key.clone(), 41);
        pooled.idle();
        ::std::thread::sleep(pool.inner.borrow().timeout);
        assert!(pool.checkout(&key).poll().unwrap().is_not_ready());
    }

    /*
    #[test]
    fn test_pool_removes_expired() {
        let mut pool = Pool::new(Duration::from_secs(1));

        let mut keep_alive = KeepAlive::new();
        keep_alive.idle();
        pool.put("foo".to_string(), (keep_alive, 41));

        let mut keep_alive = KeepAlive::new();
        keep_alive.busy();
        pool.put("foo".to_string(), (keep_alive.clone(), 5));

        assert_eq!(pool.inner.borrow().idle.get("foo").map(|entries| entries.len()), Some(2));
        ::std::thread::sleep(pool.inner.borrow().timeout);
        pool.checkout("foo");
        assert_eq!(pool.inner.borrow().idle.get("foo").map(|entries| entries.len()), Some(1));
        keep_alive.disable();
        pool.checkout("foo");
        assert!(pool.inner.borrow().idle.get("foo").is_none());
    }
    */

}
