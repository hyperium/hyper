use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use futures::{Future, Async, Poll};
use futures::sync::oneshot;
#[cfg(feature = "runtime")]
use tokio_timer::Interval;

use common::Exec;
use super::Ver;

// FIXME: allow() required due to `impl Trait` leaking types to this lint
#[allow(missing_debug_implementations)]
pub(super) struct Pool<T> {
    // If the pool is disabled, this is None.
    inner: Option<Arc<Mutex<PoolInner<T>>>>,
}

// Before using a pooled connection, make sure the sender is not dead.
//
// This is a trait to allow the `client::pool::tests` to work for `i32`.
//
// See https://github.com/hyperium/hyper/issues/1429
pub(super) trait Poolable: Send + Sized + 'static {
    fn is_open(&self) -> bool;
    /// Reserve this connection.
    ///
    /// Allows for HTTP/2 to return a shared reservation.
    fn reserve(self) -> Reservation<Self>;
    fn can_share(&self) -> bool;
}

/// When checking out a pooled connection, it might be that the connection
/// only supports a single reservation, or it might be usable for many.
///
/// Specifically, HTTP/1 requires a unique reservation, but HTTP/2 can be
/// used for multiple requests.
// FIXME: allow() required due to `impl Trait` leaking types to this lint
#[allow(missing_debug_implementations)]
pub(super) enum Reservation<T> {
    /// This connection could be used multiple times, the first one will be
    /// reinserted into the `idle` pool, and the second will be given to
    /// the `Checkout`.
    Shared(T, T),
    /// This connection requires unique access. It will be returned after
    /// use is complete.
    Unique(T),
}

/// Simple type alias in case the key type needs to be adjusted.
pub(super) type Key = Arc<String>;

struct PoolInner<T> {
    // A flag that a connection is being estabilished, and the connection
    // should be shared. This prevents making multiple HTTP/2 connections
    // to the same host.
    connecting: HashSet<Key>,
    // These are internal Conns sitting in the event loop in the KeepAlive
    // state, waiting to receive a new Request to send on the socket.
    idle: HashMap<Key, Vec<Idle<T>>>,
    max_idle_per_host: usize,
    // These are outstanding Checkouts that are waiting for a socket to be
    // able to send a Request one. This is used when "racing" for a new
    // connection.
    //
    // The Client starts 2 tasks, 1 to connect a new socket, and 1 to wait
    // for the Pool to receive an idle Conn. When a Conn becomes idle,
    // this list is checked for any parked Checkouts, and tries to notify
    // them that the Conn could be used instead of waiting for a brand new
    // connection.
    waiters: HashMap<Key, VecDeque<oneshot::Sender<T>>>,
    // A oneshot channel is used to allow the interval to be notified when
    // the Pool completely drops. That way, the interval can cancel immediately.
    #[cfg(feature = "runtime")]
    idle_interval_ref: Option<oneshot::Sender<::common::Never>>,
    #[cfg(feature = "runtime")]
    exec: Exec,
    timeout: Option<Duration>,
}

// This is because `Weak::new()` *allocates* space for `T`, even if it
// doesn't need it!
struct WeakOpt<T>(Option<Weak<T>>);

#[derive(Clone, Copy, Debug)]
pub(super) struct Config {
    pub(super) enabled: bool,
    pub(super) keep_alive_timeout: Option<Duration>,
    pub(super) max_idle_per_host: usize,
}

impl<T> Pool<T> {
    pub fn new(config: Config, __exec: &Exec) -> Pool<T> {
        let inner = if config.enabled {
             Some(Arc::new(Mutex::new(PoolInner {
                connecting: HashSet::new(),
                idle: HashMap::new(),
                #[cfg(feature = "runtime")]
                idle_interval_ref: None,
                max_idle_per_host: config.max_idle_per_host,
                waiters: HashMap::new(),
                #[cfg(feature = "runtime")]
                exec: __exec.clone(),
                timeout: config.keep_alive_timeout,
             })))
        } else {
            None
        };

        Pool {
            inner,
        }
    }

    fn is_enabled(&self) -> bool {
        self.inner.is_some()
    }

    #[cfg(test)]
    pub(super) fn no_timer(&self) {
        // Prevent an actual interval from being created for this pool...
        #[cfg(feature = "runtime")]
        {
            let mut inner = self.inner.as_ref().unwrap().lock().unwrap();
            assert!(inner.idle_interval_ref.is_none(), "timer already spawned");
            let (tx, _) = oneshot::channel();
            inner.idle_interval_ref = Some(tx);
        }
    }
}

impl<T: Poolable> Pool<T> {
    /// Returns a `Checkout` which is a future that resolves if an idle
    /// connection becomes available.
    pub fn checkout(&self, key: Key) -> Checkout<T> {
        Checkout {
            key,
            pool: self.clone(),
            waiter: None,
        }
    }

    /// Ensure that there is only ever 1 connecting task for HTTP/2
    /// connections. This does nothing for HTTP/1.
    pub(super) fn connecting(&self, key: &Key, ver: Ver) -> Option<Connecting<T>> {
        if ver == Ver::Http2 {
            if let Some(ref enabled) = self.inner {
                let mut inner = enabled.lock().unwrap();
                return if inner.connecting.insert(key.clone()) {
                    let connecting = Connecting {
                        key: key.clone(),
                        pool: WeakOpt::downgrade(enabled),
                    };
                    Some(connecting)
                } else {
                    trace!("HTTP/2 connecting already in progress for {:?}", key);
                    None
                };
            }
        }

        // else
        Some(Connecting {
            key: key.clone(),
            // in HTTP/1's case, there is never a lock, so we don't
            // need to do anything in Drop.
            pool: WeakOpt::none(),
        })
    }

    #[cfg(test)]
    fn locked(&self) -> ::std::sync::MutexGuard<PoolInner<T>> {
        self
            .inner
            .as_ref()
            .expect("enabled")
            .lock()
            .expect("lock")
    }

    #[cfg(feature = "runtime")]
    #[cfg(test)]
    pub(super) fn h1_key(&self, s: &str) -> Key {
        Arc::new(s.to_string())
    }

    #[cfg(feature = "runtime")]
    #[cfg(test)]
    pub(super) fn idle_count(&self, key: &Key) -> usize {
        self
            .locked()
            .idle
            .get(key)
            .map(|list| list.len())
            .unwrap_or(0)
    }

    pub(super) fn pooled(&self, mut connecting: Connecting<T>, value: T) -> Pooled<T> {
        let (value, pool_ref) = if let Some(ref enabled) = self.inner {
            match value.reserve() {
                Reservation::Shared(to_insert, to_return) => {
                    let mut inner = enabled.lock().unwrap();
                    inner.put(connecting.key.clone(), to_insert, enabled);
                    // Do this here instead of Drop for Connecting because we
                    // already have a lock, no need to lock the mutex twice.
                    inner.connected(&connecting.key);
                    // prevent the Drop of Connecting from repeating inner.connected()
                    connecting.pool = WeakOpt::none();

                    // Shared reservations don't need a reference to the pool,
                    // since the pool always keeps a copy.
                    (to_return, WeakOpt::none())
                },
                Reservation::Unique(value) => {
                    // Unique reservations must take a reference to the pool
                    // since they hope to reinsert once the reservation is
                    // completed
                    (value, WeakOpt::downgrade(enabled))
                },
            }
        } else {
            // If pool is not enabled, skip all the things...

            // The Connecting should have had no pool ref
            debug_assert!(connecting.pool.upgrade().is_none());

            (value, WeakOpt::none())
        };
        Pooled {
            key: connecting.key.clone(),
            is_reused: false,
            pool: pool_ref,
            value: Some(value)
        }
    }

    fn reuse(&self, key: &Key, value: T) -> Pooled<T> {
        debug!("reuse idle connection for {:?}", key);
        // TODO: unhack this
        // In Pool::pooled(), which is used for inserting brand new connections,
        // there's some code that adjusts the pool reference taken depending
        // on if the Reservation can be shared or is unique. By the time
        // reuse() is called, the reservation has already been made, and
        // we just have the final value, without knowledge of if this is
        // unique or shared. So, the hack is to just assume Ver::Http2 means
        // shared... :(
        let mut pool_ref = WeakOpt::none();
        if !value.can_share() {
            if let Some(ref enabled) = self.inner {
                pool_ref = WeakOpt::downgrade(enabled);
            }
        }

        Pooled {
            is_reused: true,
            key: key.clone(),
            pool: pool_ref,
            value: Some(value),
        }
    }
}

/// Pop off this list, looking for a usable connection that hasn't expired.
struct IdlePopper<'a, T: 'a> {
    key: &'a Key,
    list: &'a mut Vec<Idle<T>>,
}

impl<'a, T: Poolable + 'a> IdlePopper<'a, T> {
    fn pop(self, expiration: &Expiration) -> Option<Idle<T>> {
        while let Some(entry) = self.list.pop() {
            // If the connection has been closed, or is older than our idle
            // timeout, simply drop it and keep looking...
            if !entry.value.is_open() {
                trace!("removing closed connection for {:?}", self.key);
                continue;
            }
            // TODO: Actually, since the `idle` list is pushed to the end always,
            // that would imply that if *this* entry is expired, then anything
            // "earlier" in the list would *have* to be expired also... Right?
            //
            // In that case, we could just break out of the loop and drop the
            // whole list...
            if expiration.expires(entry.idle_at) {
                trace!("removing expired connection for {:?}", self.key);
                continue;
            }

            let value = match entry.value.reserve() {
                Reservation::Shared(to_reinsert, to_checkout) => {
                    self.list.push(Idle {
                        idle_at: Instant::now(),
                        value: to_reinsert,
                    });
                    to_checkout
                },
                Reservation::Unique(unique) => {
                    unique
                }
            };

            return Some(Idle {
                idle_at: entry.idle_at,
                value,
            });
        }

        None
    }
}

impl<T: Poolable> PoolInner<T> {
    fn put(&mut self, key: Key, value: T, __pool_ref: &Arc<Mutex<PoolInner<T>>>) {
        if value.can_share() && self.idle.contains_key(&key) {
            trace!("put; existing idle HTTP/2 connection for {:?}", key);
            return;
        }
        trace!("put; add idle connection for {:?}", key);
        let mut remove_waiters = false;
        let mut value = Some(value);
        if let Some(waiters) = self.waiters.get_mut(&key) {
            while let Some(tx) = waiters.pop_front() {
                if !tx.is_canceled() {
                    let reserved = value.take().expect("value already sent");
                    let reserved = match reserved.reserve() {
                        Reservation::Shared(to_keep, to_send) => {
                            value = Some(to_keep);
                            to_send
                        },
                        Reservation::Unique(uniq) => uniq,
                    };
                    match tx.send(reserved) {
                        Ok(()) => {
                            if value.is_none() {
                                break;
                            } else {
                                continue;
                            }
                        },
                        Err(e) => {
                            value = Some(e);
                        }
                    }
                }

                trace!("put; removing canceled waiter for {:?}", key);
            }
            remove_waiters = waiters.is_empty();
        }
        if remove_waiters {
            self.waiters.remove(&key);
        }

        match value {
            Some(value) => {
                // borrow-check scope...
                {
                    let idle_list = self
                        .idle
                        .entry(key.clone())
                        .or_insert(Vec::new());
                    if self.max_idle_per_host <= idle_list.len() {
                        trace!("max idle per host for {:?}, dropping connection", key);
                        return;
                    }

                    debug!("pooling idle connection for {:?}", key);
                    idle_list.push(Idle {
                        value: value,
                        idle_at: Instant::now(),
                    });
                }

                #[cfg(feature = "runtime")]
                {
                    self.spawn_idle_interval(__pool_ref);
                }
            }
            None => trace!("put; found waiter for {:?}", key),
        }
    }

    /// A `Connecting` task is complete. Not necessarily successfully,
    /// but the lock is going away, so clean up.
    fn connected(&mut self, key: &Key) {
        let existed = self.connecting.remove(key);
        debug_assert!(
            existed,
            "Connecting dropped, key not in pool.connecting"
        );
        // cancel any waiters. if there are any, it's because
        // this Connecting task didn't complete successfully.
        // those waiters would never receive a connection.
        self.waiters.remove(key);
    }

    #[cfg(feature = "runtime")]
    fn spawn_idle_interval(&mut self, pool_ref: &Arc<Mutex<PoolInner<T>>>) {
        let (dur, rx) = {
            if self.idle_interval_ref.is_some() {
                return;
            }

            if let Some(dur) = self.timeout {
                let (tx, rx) = oneshot::channel();
                self.idle_interval_ref = Some(tx);
                (dur, rx)
            } else {
                return
            }
        };

        let start = Instant::now() + dur;

        let interval = IdleInterval {
            interval: Interval::new(start, dur),
            pool: WeakOpt::downgrade(pool_ref),
            pool_drop_notifier: rx,
        };

        if let Err(err) = self.exec.execute(interval) {
            // This task isn't critical, so simply log and ignore.
            warn!("error spawning connection pool idle interval: {}", err);
        }
    }
}

impl<T> PoolInner<T> {
    /// Any `FutureResponse`s that were created will have made a `Checkout`,
    /// and possibly inserted into the pool that it is waiting for an idle
    /// connection. If a user ever dropped that future, we need to clean out
    /// those parked senders.
    fn clean_waiters(&mut self, key: &Key) {
        let mut remove_waiters = false;
        if let Some(waiters) = self.waiters.get_mut(key) {
            waiters.retain(|tx| {
                !tx.is_canceled()
            });
            remove_waiters = waiters.is_empty();
        }
        if remove_waiters {
            self.waiters.remove(key);
        }
    }
}

#[cfg(feature = "runtime")]
impl<T: Poolable> PoolInner<T> {
    /// This should *only* be called by the IdleInterval.
    fn clear_expired(&mut self) {
        let dur = self.timeout.expect("interval assumes timeout");

        let now = Instant::now();
        //self.last_idle_check_at = now;

        self.idle.retain(|key, values| {
            values.retain(|entry| {
                if !entry.value.is_open() {
                    trace!("idle interval evicting closed for {:?}", key);
                    return false;
                }
                if now - entry.idle_at > dur {
                    trace!("idle interval evicting expired for {:?}", key);
                    return false;
                }

                // Otherwise, keep this value...
                true
            });

            // returning false evicts this key/val
            !values.is_empty()
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

/// A wrapped poolable value that tries to reinsert to the Pool on Drop.
// Note: The bounds `T: Poolable` is needed for the Drop impl.
pub(super) struct Pooled<T: Poolable> {
    value: Option<T>,
    is_reused: bool,
    key: Key,
    pool: WeakOpt<Mutex<PoolInner<T>>>,
}

impl<T: Poolable> Pooled<T> {
    pub fn is_reused(&self) -> bool {
        self.is_reused
    }

    pub fn is_pool_enabled(&self) -> bool {
        self.pool.0.is_some()
    }

    fn as_ref(&self) -> &T {
        self.value.as_ref().expect("not dropped")
    }

    fn as_mut(&mut self) -> &mut T {
        self.value.as_mut().expect("not dropped")
    }
}

impl<T: Poolable> Deref for Pooled<T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.as_ref()
    }
}

impl<T: Poolable> DerefMut for Pooled<T> {
    fn deref_mut(&mut self) -> &mut T {
        self.as_mut()
    }
}

impl<T: Poolable> Drop for Pooled<T> {
    fn drop(&mut self) {
        if let Some(value) = self.value.take() {
            if !value.is_open() {
                // If we *already* know the connection is done here,
                // it shouldn't be re-inserted back into the pool.
                return;
            }

            if let Some(pool) = self.pool.upgrade() {
                if let Ok(mut inner) = pool.lock() {
                    inner.put(self.key.clone(), value, &pool);
                }
            } else if !value.can_share() {
                trace!("pool dropped, dropping pooled ({:?})", self.key);
            }
            // Ver::Http2 is already in the Pool (or dead), so we wouldn't
            // have an actual reference to the Pool.
        }
    }
}

impl<T: Poolable> fmt::Debug for Pooled<T> {
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

// FIXME: allow() required due to `impl Trait` leaking types to this lint
#[allow(missing_debug_implementations)]
pub(super) struct Checkout<T> {
    key: Key,
    pool: Pool<T>,
    waiter: Option<oneshot::Receiver<T>>,
}

impl<T: Poolable> Checkout<T> {
    fn poll_waiter(&mut self) -> Poll<Option<Pooled<T>>, ::Error> {
        static CANCELED: &str = "pool checkout failed";
        if let Some(mut rx) = self.waiter.take() {
            match rx.poll() {
                Ok(Async::Ready(value)) => {
                    if value.is_open() {
                        Ok(Async::Ready(Some(self.pool.reuse(&self.key, value))))
                    } else {
                        Err(::Error::new_canceled().with(CANCELED))
                    }
                },
                Ok(Async::NotReady) => {
                    self.waiter = Some(rx);
                    Ok(Async::NotReady)
                },
                Err(_canceled) => Err(::Error::new_canceled().with(CANCELED)),
            }
        } else {
            Ok(Async::Ready(None))
        }
    }

    fn checkout(&mut self) -> Option<Pooled<T>> {
        let entry = {
            let mut inner = self.pool.inner.as_ref()?.lock().unwrap();
            let expiration = Expiration::new(inner.timeout);
            let maybe_entry = inner.idle.get_mut(&self.key)
                .and_then(|list| {
                    trace!("take? {:?}: expiration = {:?}", self.key, expiration.0);
                    // A block to end the mutable borrow on list,
                    // so the map below can check is_empty()
                    {
                        let popper = IdlePopper {
                            key: &self.key,
                            list,
                        };
                        popper.pop(&expiration)
                    }
                        .map(|e| (e, list.is_empty()))
                });

            let (entry, empty) = if let Some((e, empty)) = maybe_entry {
                (Some(e), empty)
            } else {
                // No entry found means nuke the list for sure.
                (None, true)
            };
            if empty {
                //TODO: This could be done with the HashMap::entry API instead.
                inner.idle.remove(&self.key);
            }

            if entry.is_none() && self.waiter.is_none() {
                let (tx, mut rx) = oneshot::channel();
                let _ = rx.poll(); // park this task

                trace!("checkout waiting for idle connection: {:?}", self.key);
                inner
                    .waiters
                    .entry(self.key.clone())
                    .or_insert(VecDeque::new())
                    .push_back(tx);

                self.waiter = Some(rx);
            }

            entry
        };

        entry.map(|e| self.pool.reuse(&self.key, e.value))
    }
}

impl<T: Poolable> Future for Checkout<T> {
    type Item = Pooled<T>;
    type Error = ::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        if let Some(pooled) = try_ready!(self.poll_waiter()) {
            return Ok(Async::Ready(pooled));
        }

        if let Some(pooled) = self.checkout() {
            Ok(Async::Ready(pooled))
        } else if !self.pool.is_enabled() {
            Err(::Error::new_canceled().with("pool is disabled"))
        } else {
            Ok(Async::NotReady)
        }
    }
}

impl<T> Drop for Checkout<T> {
    fn drop(&mut self) {
        if self.waiter.take().is_some() {
            trace!("checkout dropped for {:?}", self.key);
            if let Some(Ok(mut inner)) = self.pool.inner.as_ref().map(|i| i.lock()) {
                inner.clean_waiters(&self.key);
            }
        }
    }
}

// FIXME: allow() required due to `impl Trait` leaking types to this lint
#[allow(missing_debug_implementations)]
pub(super) struct Connecting<T: Poolable> {
    key: Key,
    pool: WeakOpt<Mutex<PoolInner<T>>>,
}

impl<T: Poolable> Connecting<T> {
    pub(super) fn alpn_h2(self, pool: &Pool<T>) -> Option<Self> {
        debug_assert!(
            self.pool.0.is_none(),
            "Connecting::alpn_h2 but already Http2"
        );

        pool.connecting(&self.key, Ver::Http2)
    }
}

impl<T: Poolable> Drop for Connecting<T> {
    fn drop(&mut self) {
        if let Some(pool) = self.pool.upgrade() {
            // No need to panic on drop, that could abort!
            if let Ok(mut inner) = pool.lock() {
                inner.connected(&self.key);
            }
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

#[cfg(feature = "runtime")]
struct IdleInterval<T> {
    interval: Interval,
    pool: WeakOpt<Mutex<PoolInner<T>>>,
    // This allows the IdleInterval to be notified as soon as the entire
    // Pool is fully dropped, and shutdown. This channel is never sent on,
    // but Err(Canceled) will be received when the Pool is dropped.
    pool_drop_notifier: oneshot::Receiver<::common::Never>,
}

#[cfg(feature = "runtime")]
impl<T: Poolable + 'static> Future for IdleInterval<T> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        // Interval is a Stream
        use futures::Stream;

        loop {
            match self.pool_drop_notifier.poll() {
                Ok(Async::Ready(n)) => match n {},
                Ok(Async::NotReady) => (),
                Err(_canceled) => {
                    trace!("pool closed, canceling idle interval");
                    return Ok(Async::Ready(()));
                }
            }

            try_ready!(self.interval.poll().map_err(|err| {
                error!("idle interval timer error: {}", err);
            }));

            if let Some(inner) = self.pool.upgrade() {
                if let Ok(mut inner) = inner.lock() {
                    trace!("idle interval checking for expired");
                    inner.clear_expired();
                    continue;
                }
            }
            return Ok(Async::Ready(()));
        }
    }
}

impl<T> WeakOpt<T> {
    fn none() -> Self {
        WeakOpt(None)
    }

    fn downgrade(arc: &Arc<T>) -> Self {
        WeakOpt(Some(Arc::downgrade(arc)))
    }

    fn upgrade(&self) -> Option<Arc<T>> {
        self.0
            .as_ref()
            .and_then(Weak::upgrade)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;
    use futures::{Async, Future};
    use futures::future;
    use common::Exec;
    use super::{Connecting, Key, Poolable, Pool, Reservation, WeakOpt};

    /// Test unique reservations.
    #[derive(Debug, PartialEq, Eq)]
    struct Uniq<T>(T);

    impl<T: Send + 'static> Poolable for Uniq<T> {
        fn is_open(&self) -> bool {
            true
        }

        fn reserve(self) -> Reservation<Self> {
            Reservation::Unique(self)
        }

        fn can_share(&self) -> bool {
            false
        }
    }

    fn c<T: Poolable>(key: Key) -> Connecting<T> {
        Connecting {
            key,
            pool: WeakOpt::none(),
        }
    }

    fn pool_no_timer<T>() -> Pool<T> {
        pool_max_idle_no_timer(::std::usize::MAX)
    }

    fn pool_max_idle_no_timer<T>(max_idle: usize) -> Pool<T> {
        let pool = Pool::new(super::Config {
                enabled: true,
                keep_alive_timeout: Some(Duration::from_millis(100)),
                max_idle_per_host: max_idle,
            },
            &Exec::Default,
        );
        pool.no_timer();
        pool
    }

    #[test]
    fn test_pool_checkout_smoke() {
        let pool = pool_no_timer();
        let key = Arc::new("foo".to_string());
        let pooled = pool.pooled(c(key.clone()), Uniq(41));

        drop(pooled);

        match pool.checkout(key).poll().unwrap() {
            Async::Ready(pooled) => assert_eq!(*pooled, Uniq(41)),
            _ => panic!("not ready"),
        }
    }

    #[test]
    fn test_pool_checkout_returns_none_if_expired() {
        future::lazy(|| {
            let pool = pool_no_timer();
            let key = Arc::new("foo".to_string());
            let pooled = pool.pooled(c(key.clone()), Uniq(41));
            drop(pooled);
            ::std::thread::sleep(pool.locked().timeout.unwrap());
            assert!(pool.checkout(key).poll().unwrap().is_not_ready());
            ::futures::future::ok::<(), ()>(())
        }).wait().unwrap();
    }

    #[test]
    fn test_pool_checkout_removes_expired() {
        future::lazy(|| {
            let pool = pool_no_timer();
            let key = Arc::new("foo".to_string());

            pool.pooled(c(key.clone()), Uniq(41));
            pool.pooled(c(key.clone()), Uniq(5));
            pool.pooled(c(key.clone()), Uniq(99));

            assert_eq!(pool.locked().idle.get(&key).map(|entries| entries.len()), Some(3));
            ::std::thread::sleep(pool.locked().timeout.unwrap());

            // checkout.poll() should clean out the expired
            pool.checkout(key.clone()).poll().unwrap();
            assert!(pool.locked().idle.get(&key).is_none());

            Ok::<(), ()>(())
        }).wait().unwrap();
    }

    #[test]
    fn test_pool_max_idle_per_host() {
        future::lazy(|| {
            let pool = pool_max_idle_no_timer(2);
            let key = Arc::new("foo".to_string());

            pool.pooled(c(key.clone()), Uniq(41));
            pool.pooled(c(key.clone()), Uniq(5));
            pool.pooled(c(key.clone()), Uniq(99));

            // pooled and dropped 3, max_idle should only allow 2
            assert_eq!(pool.locked().idle.get(&key).map(|entries| entries.len()), Some(2));

            Ok::<(), ()>(())
        }).wait().unwrap();
    }

    #[cfg(feature = "runtime")]
    #[test]
    fn test_pool_timer_removes_expired() {
        use std::time::Instant;
        use tokio_timer::Delay;
        let mut rt = ::tokio::runtime::current_thread::Runtime::new().unwrap();
        let pool = Pool::new(super::Config {
                enabled: true,
                keep_alive_timeout: Some(Duration::from_millis(100)),
                max_idle_per_host: ::std::usize::MAX,
            },
            &Exec::Default,
        );

        let key = Arc::new("foo".to_string());

        // Since pool.pooled() will be calling spawn on drop, need to be sure
        // those drops are called while `rt` is the current executor. To do so,
        // call those inside a future.
        rt.block_on(::futures::future::lazy(|| {
            pool.pooled(c(key.clone()), Uniq(41));
            pool.pooled(c(key.clone()), Uniq(5));
            pool.pooled(c(key.clone()), Uniq(99));
            Ok::<_, ()>(())
        })).unwrap();

        assert_eq!(pool.locked().idle.get(&key).map(|entries| entries.len()), Some(3));

        // Let the timer tick passed the expiration...
        rt
            .block_on(Delay::new(Instant::now() + Duration::from_millis(200)))
            .expect("rt block_on 200ms");

        assert!(pool.locked().idle.get(&key).is_none());
    }

    #[test]
    fn test_pool_checkout_task_unparked() {
        let pool = pool_no_timer();
        let key = Arc::new("foo".to_string());
        let pooled = pool.pooled(c(key.clone()), Uniq(41));

        let checkout = pool.checkout(key).join(future::lazy(move || {
            // the checkout future will park first,
            // and then this lazy future will be polled, which will insert
            // the pooled back into the pool
            //
            // this test makes sure that doing so will unpark the checkout
            drop(pooled);
            Ok(())
        })).map(|(entry, _)| entry);
        assert_eq!(*checkout.wait().unwrap(), Uniq(41));
    }

    #[test]
    fn test_pool_checkout_drop_cleans_up_waiters() {
        future::lazy(|| {
            let pool = pool_no_timer::<Uniq<i32>>();
            let key = Arc::new("localhost:12345".to_string());

            let mut checkout1 = pool.checkout(key.clone());
            let mut checkout2 = pool.checkout(key.clone());

            // first poll needed to get into Pool's parked
            checkout1.poll().unwrap();
            assert_eq!(pool.locked().waiters.get(&key).unwrap().len(), 1);
            checkout2.poll().unwrap();
            assert_eq!(pool.locked().waiters.get(&key).unwrap().len(), 2);

            // on drop, clean up Pool
            drop(checkout1);
            assert_eq!(pool.locked().waiters.get(&key).unwrap().len(), 1);

            drop(checkout2);
            assert!(pool.locked().waiters.get(&key).is_none());

            ::futures::future::ok::<(), ()>(())
        }).wait().unwrap();
    }

    #[derive(Debug)]
    struct CanClose {
        val: i32,
        closed: bool,
    }

    impl Poolable for CanClose {
        fn is_open(&self) -> bool {
            !self.closed
        }

        fn reserve(self) -> Reservation<Self> {
            Reservation::Unique(self)
        }

        fn can_share(&self) -> bool {
            false
        }
    }

    #[test]
    fn pooled_drop_if_closed_doesnt_reinsert() {
        let pool = pool_no_timer();
        let key = Arc::new("localhost:12345".to_string());
        pool.pooled(c(key.clone()), CanClose {
            val: 57,
            closed: true,
        });

        assert!(!pool.locked().idle.contains_key(&key));
    }
}
