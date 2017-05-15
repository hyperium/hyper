use std::sync::{Arc, mpsc};
use std::thread;

use net::NetworkListener;

pub struct ListenerPool<A: NetworkListener> {
    acceptor: A
}

impl<A: NetworkListener + Send + 'static> ListenerPool<A> {
    /// Create a thread pool to manage the acceptor.
    pub fn new(acceptor: A) -> ListenerPool<A> {
        ListenerPool { acceptor: acceptor }
    }

    /// Runs the acceptor pool. Blocks until the acceptors are closed.
    ///
    /// ## Panics
    ///
    /// Panics if threads == 0.
    pub fn accept<F>(self, work: F, threads: usize)
        where F: Fn(A::Stream) + Send + Sync + 'static {
        assert!(threads != 0, "Can't accept on 0 threads.");

        let (super_tx, supervisor_rx) = mpsc::channel();

        let work = Arc::new(work);

        // Begin work.
        for _ in 0..threads {
            spawn_with(super_tx.clone(), work.clone(), self.acceptor.clone())
        }

        // Monitor for panics.
        // FIXME(reem): This won't ever exit since we still have a super_tx handle.
        for _ in supervisor_rx.iter() {
            spawn_with(super_tx.clone(), work.clone(), self.acceptor.clone());
        }
    }
}

fn spawn_with<A, F>(supervisor: mpsc::Sender<()>, work: Arc<F>, mut acceptor: A)
where A: NetworkListener + Send + 'static,
      F: Fn(<A as NetworkListener>::Stream) + Send + Sync + 'static {
    thread::spawn(move || {
        let _sentinel = Sentinel::new(supervisor, ());

        loop {
            match acceptor.accept() {
                Ok(stream) => work(stream),
                Err(e) => {
                    info!("Connection failed: {}", e);
                }
            }
        }
    });
}

struct Sentinel<T: Send + 'static> {
    value: Option<T>,
    supervisor: mpsc::Sender<T>,
}

impl<T: Send + 'static> Sentinel<T> {
    fn new(channel: mpsc::Sender<T>, data: T) -> Sentinel<T> {
        Sentinel {
            value: Some(data),
            supervisor: channel,
        }
    }
}

impl<T: Send + 'static> Drop for Sentinel<T> {
    fn drop(&mut self) {
        // Respawn ourselves
        let _ = self.supervisor.send(self.value.take().unwrap());
    }
}

