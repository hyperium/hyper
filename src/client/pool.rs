use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use futures::{Future, Async, Poll, Stream};
use futures::sync::oneshot;
use futures_timer::Interval;

use common::Never;
use super::Exec;

pub struct Pool<T> {
    inner: Arc<Mutex<PoolInner<T>>>,
}

// Before using a pooled connection, make sure the sender is not dead.
//
// This is a trait to allow the `client::pool::tests` to work for `i32`.
//
// See https://github.com/hyperium/hyper/issues/1429
pub trait Closed {
    fn is_closed(&self) -> bool;
}

struct PoolInner<T> {
    enabled: bool,
    // These are internal Conns sitting in the event loop in the KeepAlive
    // state, waiting to receive a new Request to send on the socket.
    idle: HashMap<Arc<String>, Vec<Idle<T>>>,
    // These are outstanding Checkouts that are waiting for a socket to be
    // able to send a Request one. This is used when "racing" for a new
    // connection.
    //
    // The Client starts 2 tasks, 1 to connect a new socket, and 1 to wait
    // for the Pool to receive an idle Conn. When a Conn becomes idle,
    // this list is checked for any parked Checkouts, and tries to notify
    // them that the Conn could be used instead of waiting for a brand new
    // connection.
    parked: HashMap<Arc<String>, VecDeque<oneshot::Sender<T>>>,
    timeout: Option<Duration>,
    // A oneshot channel is used to allow the interval to be notified when
    // the Pool completely drops. That way, the interval can cancel immediately.
    idle_interval_ref: Option<oneshot::Sender<Never>>,
}

impl<T> Pool<T> {
    pub fn new(enabled: bool, timeout: Option<Duration>) -> Pool<T> {
        Pool {
            inner: Arc::new(Mutex::new(PoolInner {
                enabled: enabled,
                idle: HashMap::new(),
                idle_interval_ref: None,
                parked: HashMap::new(),
                timeout: timeout,
            })),
        }
    }
}

impl<T: Closed> Pool<T> {
    pub fn checkout(&self, key: &str) -> Checkout<T> {
        Checkout {
            key: Arc::new(key.to_owned()),
            pool: self.clone(),
            parked: None,
        }
    }

    fn take(&self, key: &Arc<String>) -> Option<Pooled<T>> {
        let entry = {
            let mut inner = self.inner.lock().unwrap();
            let expiration = Expiration::new(inner.timeout);
            let mut should_remove = false;
            let entry = inner.idle.get_mut(key).and_then(|list| {
                trace!("take; url = {:?}, expiration = {:?}", key, expiration.0);
                while let Some(entry) = list.pop() {
                    if !expiration.expires(entry.idle_at) {
                        if !entry.value.is_closed() {
                            should_remove = list.is_empty();
                            return Some(entry);
                        }
                    }
                    trace!("removing unacceptable pooled {:?}", key);
                    // every other case the Idle should just be dropped
                    // 1. Idle but expired
                    // 2. Busy (something else somehow took it?)
                    // 3. Disabled don't reuse of course
                }
                should_remove = true;
                None
            });

            if should_remove {
                inner.idle.remove(key);
            }
            entry
        };

        entry.map(|e| self.reuse(key, e.value))
    }


    pub fn pooled(&self, key: Arc<String>, value: T) -> Pooled<T> {
        Pooled {
            is_reused: false,
            key: key,
            pool: Arc::downgrade(&self.inner),
            value: Some(value)
        }
    }

    fn reuse(&self, key: &Arc<String>, value: T) -> Pooled<T> {
        debug!("reuse idle connection for {:?}", key);
        Pooled {
            is_reused: true,
            key: key.clone(),
            pool: Arc::downgrade(&self.inner),
            value: Some(value),
        }
    }

    fn park(&mut self, key: Arc<String>, tx: oneshot::Sender<T>) {
        trace!("park; waiting for idle connection: {:?}", key);
        self.inner.lock().unwrap()
            .parked.entry(key)
            .or_insert(VecDeque::new())
            .push_back(tx);
    }
}

impl<T: Closed> PoolInner<T> {
    fn put(&mut self, key: Arc<String>, value: T) {
        if !self.enabled {
            return;
        }
        trace!("Pool::put {:?}", key);
        let mut remove_parked = false;
        let mut value = Some(value);
        if let Some(parked) = self.parked.get_mut(&key) {
            while let Some(tx) = parked.pop_front() {
                if !tx.is_canceled() {
                    match tx.send(value.take().unwrap()) {
                        Ok(()) => break,
                        Err(e) => {
                            value = Some(e);
                        }
                    }
                }

                trace!("Pool::put removing canceled parked {:?}", key);
            }
            remove_parked = parked.is_empty();
        }
        if remove_parked {
            self.parked.remove(&key);
        }

        match value {
            Some(value) => {
                debug!("pooling idle connection for {:?}", key);
                self.idle.entry(key)
                     .or_insert(Vec::new())
                     .push(Idle {
                         value: value,
                         idle_at: Instant::now(),
                     });
            }
            None => trace!("Pool::put found parked {:?}", key),
        }
    }
}

impl<T> PoolInner<T> {
    /// Any `FutureResponse`s that were created will have made a `Checkout`,
    /// and possibly inserted into the pool that it is waiting for an idle
    /// connection. If a user ever dropped that future, we need to clean out
    /// those parked senders.
    fn clean_parked(&mut self, key: &Arc<String>) {
        let mut remove_parked = false;
        if let Some(parked) = self.parked.get_mut(key) {
            parked.retain(|tx| {
                !tx.is_canceled()
            });
            remove_parked = parked.is_empty();
        }
        if remove_parked {
            self.parked.remove(key);
        }
    }
}

impl<T: Closed> PoolInner<T> {
    fn clear_expired(&mut self) {
        let dur = if let Some(dur) = self.timeout {
            dur
        } else {
            return
        };

        let now = Instant::now();
        //self.last_idle_check_at = now;

        self.idle.retain(|_key, values| {

            values.retain(|entry| {
                if entry.value.is_closed() {
                    return false;
                }
                now - entry.idle_at < dur
            });

            // returning false evicts this key/val
            !values.is_empty()
        });
    }
}


impl<T: Closed + Send + 'static> Pool<T> {
    pub(super) fn spawn_expired_interval(&self, exec: &Exec) {
        let (dur, rx) = {
            let mut inner = self.inner.lock().unwrap();

            if !inner.enabled {
                return;
            }

            if inner.idle_interval_ref.is_some() {
                return;
            }

            if let Some(dur) = inner.timeout {
                let (tx, rx) = oneshot::channel();
                inner.idle_interval_ref = Some(tx);
                (dur, rx)
            } else {
                return
            }
        };

        let interval = Interval::new(dur);
        exec.execute(IdleInterval {
            interval: interval,
            pool: Arc::downgrade(&self.inner),
            pool_drop_notifier: rx,
        });
    }
}

impl<T> Clone for Pool<T> {
    fn clone(&self) -> Pool<T> {
        Pool {
            inner: self.inner.clone(),
        }
    }
}

pub struct Pooled<T: Closed> {
    value: Option<T>,
    is_reused: bool,
    key: Arc<String>,
    pool: Weak<Mutex<PoolInner<T>>>,
}

impl<T: Closed> Pooled<T> {
    pub fn is_reused(&self) -> bool {
        self.is_reused
    }

    fn as_ref(&self) -> &T {
        self.value.as_ref().expect("not dropped")
    }

    fn as_mut(&mut self) -> &mut T {
        self.value.as_mut().expect("not dropped")
    }
}

impl<T: Closed> Deref for Pooled<T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.as_ref()
    }
}

impl<T: Closed> DerefMut for Pooled<T> {
    fn deref_mut(&mut self) -> &mut T {
        self.as_mut()
    }
}

impl<T: Closed> Drop for Pooled<T> {
    fn drop(&mut self) {
        if let Some(value) = self.value.take() {
            if let Some(inner) = self.pool.upgrade() {
                if let Ok(mut inner) = inner.lock() {
                    inner.put(self.key.clone(), value);
                }
            } else {
                trace!("pool dropped, dropping pooled ({:?})", self.key);
            }
        }
    }
}

impl<T: Closed> fmt::Debug for Pooled<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Pooled")
            .field("key", &self.key)
            .finish()
    }
}

struct Idle<T> {
    idle_at: Instant,
    value: T,
}

pub struct Checkout<T> {
    key: Arc<String>,
    pool: Pool<T>,
    parked: Option<oneshot::Receiver<T>>,
}

struct NotParked;

impl<T: Closed> Checkout<T> {
    fn poll_parked(&mut self) -> Poll<Pooled<T>, NotParked> {
        let mut drop_parked = false;
        if let Some(ref mut rx) = self.parked {
            match rx.poll() {
                Ok(Async::Ready(value)) => {
                    if !value.is_closed() {
                        return Ok(Async::Ready(self.pool.reuse(&self.key, value)));
                    }
                    drop_parked = true;
                },
                Ok(Async::NotReady) => return Ok(Async::NotReady),
                Err(_canceled) => drop_parked = true,
            }
        }
        if drop_parked {
            self.parked.take();
        }
        Err(NotParked)
    }

    fn park(&mut self) {
        if self.parked.is_none() {
            let (tx, mut rx) = oneshot::channel();
            let _ = rx.poll(); // park this task
            self.pool.park(self.key.clone(), tx);
            self.parked = Some(rx);
        }
    }
}

impl<T: Closed> Future for Checkout<T> {
    type Item = Pooled<T>;
    type Error = ::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match self.poll_parked() {
            Ok(async) => return Ok(async),
            Err(_not_parked) => (),
        }

        let entry = self.pool.take(&self.key);

        if let Some(pooled) = entry {
            Ok(Async::Ready(pooled))
        } else {
            self.park();
            Ok(Async::NotReady)
        }
    }
}

impl<T> Drop for Checkout<T> {
    fn drop(&mut self) {
        self.parked.take();
        if let Ok(mut inner) = self.pool.inner.lock() {
            inner.clean_parked(&self.key);
        }
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

struct IdleInterval<T> {
    interval: Interval,
    pool: Weak<Mutex<PoolInner<T>>>,
    // This allows the IdleInterval to be notified as soon as the entire
    // Pool is fully dropped, and shutdown. This channel is never sent on,
    // but Err(Canceled) will be received when the Pool is dropped.
    pool_drop_notifier: oneshot::Receiver<Never>,
}

impl<T: Closed + 'static> Future for IdleInterval<T> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            match self.pool_drop_notifier.poll() {
                Ok(Async::Ready(n)) => match n {},
                Ok(Async::NotReady) => (),
                Err(_canceled) => {
                    trace!("pool closed, canceling idle interval");
                    return Ok(Async::Ready(()));
                }
            }

            try_ready!(self.interval.poll().map_err(|_| unreachable!("interval cannot error")));

            if let Some(inner) = self.pool.upgrade() {
                if let Ok(mut inner) = inner.lock() {
                    inner.clear_expired();
                    continue;
                }
            }
            return Ok(Async::Ready(()));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;
    use futures::{Async, Future};
    use futures::future;
    use super::{Closed, Pool, Exec};

    impl Closed for i32 {
        fn is_closed(&self) -> bool {
            false
        }
    }

    #[test]
    fn test_pool_checkout_smoke() {
        let pool = Pool::new(true, Some(Duration::from_secs(5)));
        let key = Arc::new("foo".to_string());
        let pooled = pool.pooled(key.clone(), 41);

        drop(pooled);

        match pool.checkout(&key).poll().unwrap() {
            Async::Ready(pooled) => assert_eq!(*pooled, 41),
            _ => panic!("not ready"),
        }
    }

    #[test]
    fn test_pool_checkout_returns_none_if_expired() {
        future::lazy(|| {
            let pool = Pool::new(true, Some(Duration::from_millis(100)));
            let key = Arc::new("foo".to_string());
            let pooled = pool.pooled(key.clone(), 41);
            drop(pooled);
            ::std::thread::sleep(pool.inner.lock().unwrap().timeout.unwrap());
            assert!(pool.checkout(&key).poll().unwrap().is_not_ready());
            ::futures::future::ok::<(), ()>(())
        }).wait().unwrap();
    }

    #[test]
    fn test_pool_checkout_removes_expired() {
        future::lazy(|| {
            let pool = Pool::new(true, Some(Duration::from_millis(100)));
            let key = Arc::new("foo".to_string());

            pool.pooled(key.clone(), 41);
            pool.pooled(key.clone(), 5);
            pool.pooled(key.clone(), 99);

            assert_eq!(pool.inner.lock().unwrap().idle.get(&key).map(|entries| entries.len()), Some(3));
            ::std::thread::sleep(pool.inner.lock().unwrap().timeout.unwrap());

            // checkout.poll() should clean out the expired
            pool.checkout(&key).poll().unwrap();
            assert!(pool.inner.lock().unwrap().idle.get(&key).is_none());

            Ok::<(), ()>(())
        }).wait().unwrap();
    }

    #[test]
    fn test_pool_timer_removes_expired() {
        let runtime = ::tokio::runtime::Runtime::new().unwrap();
        let pool = Pool::new(true, Some(Duration::from_millis(100)));

        let executor = runtime.executor();
        pool.spawn_expired_interval(&Exec::new(executor));
        let key = Arc::new("foo".to_string());

        pool.pooled(key.clone(), 41);
        pool.pooled(key.clone(), 5);
        pool.pooled(key.clone(), 99);

        assert_eq!(pool.inner.lock().unwrap().idle.get(&key).map(|entries| entries.len()), Some(3));

        ::futures_timer::Delay::new(
            Duration::from_millis(400) // allow for too-good resolution
        ).wait().unwrap();

        assert!(pool.inner.lock().unwrap().idle.get(&key).is_none());
    }

    #[test]
    fn test_pool_checkout_task_unparked() {
        let pool = Pool::new(true, Some(Duration::from_secs(10)));
        let key = Arc::new("foo".to_string());
        let pooled = pool.pooled(key.clone(), 41);

        let checkout = pool.checkout(&key).join(future::lazy(move || {
            // the checkout future will park first,
            // and then this lazy future will be polled, which will insert
            // the pooled back into the pool
            //
            // this test makes sure that doing so will unpark the checkout
            drop(pooled);
            Ok(())
        })).map(|(entry, _)| entry);
        assert_eq!(*checkout.wait().unwrap(), 41);
    }

    #[test]
    fn test_pool_checkout_drop_cleans_up_parked() {
        future::lazy(|| {
            let pool = Pool::<i32>::new(true, Some(Duration::from_secs(10)));
            let key = Arc::new("localhost:12345".to_string());

            let mut checkout1 = pool.checkout(&key);
            let mut checkout2 = pool.checkout(&key);

            // first poll needed to get into Pool's parked
            checkout1.poll().unwrap();
            assert_eq!(pool.inner.lock().unwrap().parked.get(&key).unwrap().len(), 1);
            checkout2.poll().unwrap();
            assert_eq!(pool.inner.lock().unwrap().parked.get(&key).unwrap().len(), 2);

            // on drop, clean up Pool
            drop(checkout1);
            assert_eq!(pool.inner.lock().unwrap().parked.get(&key).unwrap().len(), 1);

            drop(checkout2);
            assert!(pool.inner.lock().unwrap().parked.get(&key).is_none());

            ::futures::future::ok::<(), ()>(())
        }).wait().unwrap();
    }
}
