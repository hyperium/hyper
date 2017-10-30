use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::io;
use std::ops::{Deref, DerefMut, BitAndAssign};
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};

use futures::{Future, Async, Poll};
use relay;

use proto::{KeepAlive, KA};

pub struct Pool<T> {
    inner: Rc<RefCell<PoolInner<T>>>,
}

struct PoolInner<T> {
    enabled: bool,
    // These are internal Conns sitting in the event loop in the KeepAlive
    // state, waiting to receive a new Request to send on the socket.
    idle: HashMap<Rc<String>, Vec<Entry<T>>>,
    // These are outstanding Checkouts that are waiting for a socket to be
    // able to send a Request one. This is used when "racing" for a new
    // connection.
    //
    // The Client starts 2 tasks, 1 to connect a new socket, and 1 to wait
    // for the Pool to receive an idle Conn. When a Conn becomes idle,
    // this list is checked for any parked Checkouts, and tries to notify
    // them that the Conn could be used instead of waiting for a brand new
    // connection.
    parked: HashMap<Rc<String>, VecDeque<relay::Sender<Entry<T>>>>,
    timeout: Option<Duration>,
}

impl<T: Clone> Pool<T> {
    pub fn new(enabled: bool, timeout: Option<Duration>) -> Pool<T> {
        Pool {
            inner: Rc::new(RefCell::new(PoolInner {
                enabled: enabled,
                idle: HashMap::new(),
                parked: HashMap::new(),
                timeout: timeout,
            })),
        }
    }

    pub fn checkout(&self, key: &str) -> Checkout<T> {
        Checkout {
            key: Rc::new(key.to_owned()),
            pool: self.clone(),
            parked: None,
        }
    }

    fn put(&mut self, key: Rc<String>, entry: Entry<T>) {
        trace!("Pool::put {:?}", key);
        let mut inner = self.inner.borrow_mut();
        //let inner = &mut *inner;
        let mut remove_parked = false;
        let mut entry = Some(entry);
        if let Some(parked) = inner.parked.get_mut(&key) {
            while let Some(tx) = parked.pop_front() {
                if tx.is_canceled() {
                    trace!("Pool::put removing canceled parked {:?}", key);
                } else {
                    tx.complete(entry.take().unwrap());
                    break;
                }
                /*
                match tx.send(entry.take().unwrap()) {
                    Ok(()) => break,
                    Err(e) => {
                        trace!("Pool::put removing canceled parked {:?}", key);
                        entry = Some(e);
                    }
                }
                */
            }
            remove_parked = parked.is_empty();
        }
        if remove_parked {
            inner.parked.remove(&key);
        }

        match entry {
            Some(entry) => {
                inner.idle.entry(key)
                     .or_insert(Vec::new())
                     .push(entry);
            }
            None => trace!("Pool::put found parked {:?}", key),
        }
    }


    pub fn pooled(&self, key: Rc<String>, value: T) -> Pooled<T> {
        trace!("Pool::pooled {:?}", key);
        Pooled {
            entry: Entry {
                value: value,
                is_reused: false,
                status: Rc::new(Cell::new(TimedKA::Busy)),
            },
            key: key,
            pool: Rc::downgrade(&self.inner),
        }
    }

    fn is_enabled(&self) -> bool {
        self.inner.borrow().enabled
    }

    fn reuse(&self, key: Rc<String>, mut entry: Entry<T>) -> Pooled<T> {
        trace!("Pool::reuse {:?}", key);
        entry.is_reused = true;
        entry.status.set(TimedKA::Busy);
        Pooled {
            entry: entry,
            key: key,
            pool: Rc::downgrade(&self.inner),
        }
    }

    fn park(&mut self, key: Rc<String>, tx: relay::Sender<Entry<T>>) {
        trace!("Pool::park {:?}", key);
        self.inner.borrow_mut()
            .parked.entry(key)
            .or_insert(VecDeque::new())
            .push_back(tx);
    }
}

impl<T> Pool<T> {
    fn clean_parked(&mut self, key: &Rc<String>) {
        trace!("Pool::clean_parked {:?}", key);
        let mut inner = self.inner.borrow_mut();

        let mut remove_parked = false;
        if let Some(parked) = inner.parked.get_mut(key) {
            parked.retain(|tx| {
                !tx.is_canceled()
            });
            remove_parked = parked.is_empty();
        }
        if remove_parked {
            inner.parked.remove(key);
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
    pool: Weak<RefCell<PoolInner<T>>>,
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
        self.entry.status.set(TimedKA::Busy);
    }

    fn disable(&mut self) {
        self.entry.status.set(TimedKA::Disabled);
    }

    fn idle(&mut self) {
        let previous = self.status();
        self.entry.status.set(TimedKA::Idle(Instant::now()));
        if let KA::Idle = previous {
            trace!("Pooled::idle already idle");
            return;
        }
        self.entry.is_reused = true;
        if let Some(inner) = self.pool.upgrade() {
            let mut pool = Pool {
                inner: inner,
            };
            if pool.is_enabled() {
                pool.put(self.key.clone(), self.entry.clone());
            }
        } else {
            trace!("pool dropped, dropping pooled ({:?})", self.key);
            self.entry.status.set(TimedKA::Disabled);
        }
    }

    fn status(&self) -> KA {
        match self.entry.status.get() {
            TimedKA::Idle(_) => KA::Idle,
            TimedKA::Busy => KA::Busy,
            TimedKA::Disabled => KA::Disabled,
        }
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
    status: Rc<Cell<TimedKA>>,
}

#[derive(Clone, Copy, Debug)]
enum TimedKA {
    Idle(Instant),
    Busy,
    Disabled,
}

pub struct Checkout<T> {
    key: Rc<String>,
    pool: Pool<T>,
    parked: Option<relay::Receiver<Entry<T>>>,
}

impl<T: Clone> Future for Checkout<T> {
    type Item = Pooled<T>;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        trace!("Checkout::poll");
        let mut drop_parked = false;
        if let Some(ref mut rx) = self.parked {
            match rx.poll() {
                Ok(Async::Ready(entry)) => {
                    trace!("Checkout::poll found client in relay for {:?}", self.key);
                    return Ok(Async::Ready(self.pool.reuse(self.key.clone(), entry)));
                },
                Ok(Async::NotReady) => (),
                Err(_canceled) => drop_parked = true,
            }
        }
        if drop_parked {
            self.parked.take();
        }
        let expiration = Expiration::new(self.pool.inner.borrow().timeout);
        let key = &self.key;
        trace!("Checkout::poll url = {:?}, expiration = {:?}", key, expiration.0);
        let mut should_remove = false;
        let entry = self.pool.inner.borrow_mut().idle.get_mut(key).and_then(|list| {
            trace!("Checkout::poll key found {:?}", key);
            while let Some(entry) = list.pop() {
                match entry.status.get() {
                    TimedKA::Idle(idle_at) if !expiration.expires(idle_at) => {
                        trace!("Checkout::poll found idle client for {:?}", key);
                        should_remove = list.is_empty();
                        return Some(entry);
                    },
                    _ => {
                        trace!("Checkout::poll removing unacceptable pooled {:?}", key);
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
            None => {
                if self.parked.is_none() {
                    let (tx, mut rx) = relay::channel();
                    let _ = rx.poll(); // park this task
                    self.pool.park(self.key.clone(), tx);
                    self.parked = Some(rx);
                }
                Ok(Async::NotReady)
            },
        }
    }
}

impl<T> Drop for Checkout<T> {
    fn drop(&mut self) {
        self.parked.take();
        self.pool.clean_parked(&self.key);
    }
}

struct Expiration(Option<Duration>);

impl Expiration {
    fn new(dur: Option<Duration>) -> Expiration {
        Expiration(dur)
    }

    fn expires(&self, instant: Instant) -> bool {
        match self.0 {
            Some(timeout) => instant.elapsed() > timeout,
            None => false,
        }
    }
}


#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use std::time::Duration;
    use futures::{Async, Future};
    use futures::future;
    use proto::KeepAlive;
    use super::Pool;

    #[test]
    fn test_pool_checkout_smoke() {
        let pool = Pool::new(true, Some(Duration::from_secs(5)));
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
        future::lazy(|| {
            let pool = Pool::new(true, Some(Duration::from_secs(1)));
            let key = Rc::new("foo".to_string());
            let mut pooled = pool.pooled(key.clone(), 41);
            pooled.idle();
            ::std::thread::sleep(pool.inner.borrow().timeout.unwrap());
            assert!(pool.checkout(&key).poll().unwrap().is_not_ready());
            ::futures::future::ok::<(), ()>(())
        }).wait().unwrap();
    }

    #[test]
    fn test_pool_removes_expired() {
        let pool = Pool::new(true, Some(Duration::from_secs(1)));
        let key = Rc::new("foo".to_string());

        let mut pooled1 = pool.pooled(key.clone(), 41);
        pooled1.idle();
        let mut pooled2 = pool.pooled(key.clone(), 5);
        pooled2.idle();
        let mut pooled3 = pool.pooled(key.clone(), 99);
        pooled3.idle();


        assert_eq!(pool.inner.borrow().idle.get(&key).map(|entries| entries.len()), Some(3));
        ::std::thread::sleep(pool.inner.borrow().timeout.unwrap());

        pooled1.idle();
        pooled2.idle(); // idle after sleep, not expired
        pool.checkout(&key).poll().unwrap();
        assert_eq!(pool.inner.borrow().idle.get(&key).map(|entries| entries.len()), Some(1));
        pool.checkout(&key).poll().unwrap();
        assert!(pool.inner.borrow().idle.get(&key).is_none());
    }

    #[test]
    fn test_pool_checkout_task_unparked() {
        let pool = Pool::new(true, Some(Duration::from_secs(10)));
        let key = Rc::new("foo".to_string());
        let pooled1 = pool.pooled(key.clone(), 41);

        let mut pooled = pooled1.clone();
        let checkout = pool.checkout(&key).join(future::lazy(move || {
            // the checkout future will park first,
            // and then this lazy future will be polled, which will insert
            // the pooled back into the pool
            //
            // this test makes sure that doing so will unpark the checkout
            pooled.idle();
            Ok(())
        })).map(|(entry, _)| entry);
        assert_eq!(*checkout.wait().unwrap(), *pooled1);
    }

    #[test]
    fn test_pool_checkout_drop_cleans_up_parked() {
        future::lazy(|| {
            let pool = Pool::new(true, Some(Duration::from_secs(10)));
            let key = Rc::new("localhost:12345".to_string());
            let _pooled1 = pool.pooled(key.clone(), 41);
            let mut checkout1 = pool.checkout(&key);
            let mut checkout2 = pool.checkout(&key);

            // first poll needed to get into Pool's parked
            checkout1.poll().unwrap();
            assert_eq!(pool.inner.borrow().parked.get(&key).unwrap().len(), 1);
            checkout2.poll().unwrap();
            assert_eq!(pool.inner.borrow().parked.get(&key).unwrap().len(), 2);

            // on drop, clean up Pool
            drop(checkout1);
            assert_eq!(pool.inner.borrow().parked.get(&key).unwrap().len(), 1);

            drop(checkout2);
            assert!(pool.inner.borrow().parked.get(&key).is_none());

            ::futures::future::ok::<(), ()>(())
        }).wait().unwrap();
    }
}
