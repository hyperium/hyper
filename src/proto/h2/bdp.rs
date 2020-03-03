// What should it do?
//
// # BDP Algorithm
//
// 1. When receiving a DATA frame, if a BDP ping isn't outstanding:
//   1a. Record current time.
//   1b. Send a BDP ping.
// 2. Increment the number of received bytes.
// 3. When the BDP ping ack is received:
//   3a. Record duration from sent time.
//   3b. Merge RTT with a running average.
//   3c. Calculate bdp as bytes/rtt.
//   3d. If bdp is over 2/3 max, set new max to bdp and update windows.
//
//
// # Implementation
//
// - `hyper::Body::h2` variant includes a "bdp channel"
//   - When the body's `poll_data` yields bytes, call `bdp.sample(bytes.len())`
//

use std::sync::{Arc, Mutex, Weak};
use std::task::{self, Poll};
use std::time::{Duration, Instant};

use h2::{Ping, PingPong};

type WindowSize = u32;

/// Any higher than this likely will be hitting the TCP flow control.
const BDP_LIMIT: usize = 1024 * 1024 * 16;

pub(super) fn disabled() -> Sampler {
    Sampler {
        shared: Weak::new(),
    }
}

pub(super) fn channel(ping_pong: PingPong, initial_window: WindowSize) -> (Sampler, Estimator) {
    let shared = Arc::new(Mutex::new(Shared {
        bytes: 0,
        ping_pong,
        ping_sent: false,
        sent_at: Instant::now(),
    }));

    (
        Sampler {
            shared: Arc::downgrade(&shared),
        },
        Estimator {
            bdp: initial_window,
            max_bandwidth: 0.0,
            shared,
            samples: 0,
            rtt: 0.0,
        },
    )
}

#[derive(Clone)]
pub(crate) struct Sampler {
    shared: Weak<Mutex<Shared>>,
}

pub(super) struct Estimator {
    shared: Arc<Mutex<Shared>>,

    /// Current BDP in bytes
    bdp: u32,
    /// Largest bandwidth we've seen so far.
    max_bandwidth: f64,
    /// Count of samples made (ping sent and received)
    samples: usize,
    /// Round trip time in seconds
    rtt: f64,
}

struct Shared {
    bytes: usize,
    ping_pong: PingPong,
    ping_sent: bool,
    sent_at: Instant,
}

impl Sampler {
    pub(crate) fn sample(&self, bytes: usize) {
        let shared = if let Some(shared) = self.shared.upgrade() {
            shared
        } else {
            return;
        };

        let mut inner = shared.lock().unwrap();

        if !inner.ping_sent {
            if let Ok(()) = inner.ping_pong.send_ping(Ping::opaque()) {
                inner.ping_sent = true;
                inner.sent_at = Instant::now();
                trace!("sending BDP ping");
            } else {
                return;
            }
        }

        inner.bytes += bytes;
    }
}

impl Estimator {
    pub(super) fn poll_estimate(&mut self, cx: &mut task::Context<'_>) -> Poll<WindowSize> {
        let mut inner = self.shared.lock().unwrap();
        if !inner.ping_sent {
            // XXX: this doesn't register a waker...?
            return Poll::Pending;
        }

        let (bytes, rtt) = match ready!(inner.ping_pong.poll_pong(cx)) {
            Ok(_pong) => {
                let rtt = inner.sent_at.elapsed();
                let bytes = inner.bytes;
                inner.bytes = 0;
                inner.ping_sent = false;
                self.samples += 1;
                trace!("received BDP ack; bytes = {}, rtt = {:?}", bytes, rtt);
                (bytes, rtt)
            }
            Err(e) => {
                debug!("bdp pong error: {}", e);
                return Poll::Pending;
            }
        };

        drop(inner);

        if let Some(bdp) = self.calculate(bytes, rtt) {
            Poll::Ready(bdp)
        } else {
            // XXX: this doesn't register a waker...?
            Poll::Pending
        }
    }

    fn calculate(&mut self, bytes: usize, rtt: Duration) -> Option<WindowSize> {
        // No need to do any math if we're at the limit.
        if self.bdp as usize == BDP_LIMIT {
            return None;
        }

        // average the rtt
        let rtt = seconds(rtt);
        if self.samples < 10 {
            // Average the first 10 samples
            self.rtt += (rtt - self.rtt) / (self.samples as f64);
        } else {
            self.rtt += (rtt - self.rtt) / 0.9;
        }

        // calculate the current bandwidth
        let bw = (bytes as f64) / (self.rtt * 1.5);
        trace!("current bandwidth = {:.1}B/s", bw);

        if bw < self.max_bandwidth {
            // not a faster bandwidth, so don't update
            return None;
        } else {
            self.max_bandwidth = bw;
        }

        // if the current `bytes` sample is at least 2/3 the previous
        // bdp, increase to double the current sample.
        if (bytes as f64) >= (self.bdp as f64) * 0.66 {
            self.bdp = (bytes * 2).min(BDP_LIMIT) as WindowSize;
            trace!("BDP increased to {}", self.bdp);
            Some(self.bdp)
        } else {
            None
        }
    }
}

fn seconds(dur: Duration) -> f64 {
    const NANOS_PER_SEC: f64 = 1_000_000_000.0;
    let secs = dur.as_secs() as f64;
    secs + (dur.subsec_nanos() as f64) / NANOS_PER_SEC
}
