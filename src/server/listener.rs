use std::thread::{self, JoinGuard};
use std::sync::mpsc;
use net::NetworkListener;

pub struct ListenerPool<A: NetworkListener> {
    acceptor: A
}

impl<'a, A: NetworkListener + Send + 'a> ListenerPool<A> {
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
        where F: Fn(A::Stream) + Send + Sync + 'a {
        assert!(threads != 0, "Can't accept on 0 threads.");

        let (super_tx, supervisor_rx) = mpsc::channel();

        let work = &work;
        let spawn = move |id| {
            spawn_with(super_tx.clone(), work, self.acceptor.clone(), id)
        };

        // Go
        let mut guards: Vec<_> = (0..threads).map(|id| spawn(id)).collect();

        for id in supervisor_rx.iter() {
            guards[id] = spawn(id);
        }
    }
}

fn spawn_with<'a, A, F>(supervisor: mpsc::Sender<usize>, work: &'a F, mut acceptor: A, id: usize) -> thread::JoinGuard<'a, ()>
where A: NetworkListener + Send + 'a,
      F: Fn(<A as NetworkListener>::Stream) + Send + Sync + 'a {

    thread::scoped(move || {
        let _sentinel = Sentinel::new(supervisor, id);

        loop {
            match acceptor.accept() {
                Ok(stream) => work(stream),
                Err(e) => {
                    error!("Connection failed: {}", e);
                }
            }
        }
    })
}

struct Sentinel<T: Send + 'static> {
    value: Option<T>,
    supervisor: mpsc::Sender<T>,
    //active: bool
}

impl<T: Send + 'static> Sentinel<T> {
    fn new(channel: mpsc::Sender<T>, data: T) -> Sentinel<T> {
        Sentinel {
            value: Some(data),
            supervisor: channel,
            //active: true
        }
    }

    //fn cancel(mut self) { self.active = false; }
}

impl<T: Send + 'static> Drop for Sentinel<T> {
    fn drop(&mut self) {
        // If we were cancelled, get out of here.
        //if !self.active { return; }

        // Respawn ourselves
        let _ = self.supervisor.send(self.value.take().unwrap());
    }
}

