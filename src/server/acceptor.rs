use std::thread::{self, JoinGuard};
use std::sync::mpsc;
use std::collections::VecMap;
use net::NetworkAcceptor;

pub struct AcceptorPool<A: NetworkAcceptor> {
    acceptor: A
}

impl<'a, A: NetworkAcceptor + 'a> AcceptorPool<A> {
    /// Create a thread pool to manage the acceptor.
    pub fn new(acceptor: A) -> AcceptorPool<A> {
        AcceptorPool { acceptor: acceptor }
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

        let counter = &mut 0;
        let work = &work;
        let mut spawn = move || {
            let id = *counter;
            let guard = spawn_with(super_tx.clone(), work, self.acceptor.clone(), id);
            *counter += 1;
            (id, guard)
        };

        // Go
        let mut guards: VecMap<_> = (0..threads).map(|_| spawn()).collect();

        for id in supervisor_rx.iter() {
            guards.remove(&id);
            let (id, guard) = spawn();
            guards.insert(id, guard);
        }
    }
}

fn spawn_with<'a, A, F>(supervisor: mpsc::Sender<usize>, work: &'a F, mut acceptor: A, id: usize) -> JoinGuard<'a, ()>
where A: NetworkAcceptor + 'a,
      F: Fn(<A as NetworkAcceptor>::Stream) + Send + Sync + 'a {
    use std::old_io::EndOfFile;

    thread::scoped(move || {
        let sentinel = Sentinel::new(supervisor, id);

        loop {
            match acceptor.accept() {
                Ok(stream) => work(stream),
                Err(ref e) if e.kind == EndOfFile => {
                    debug!("Server closed.");
                    sentinel.cancel();
                    return;
                },

                Err(e) => {
                    error!("Connection failed: {}", e);
                }
            }
        }
    })
}

struct Sentinel<T: Send> {
    value: Option<T>,
    supervisor: mpsc::Sender<T>,
    active: bool
}

impl<T: Send> Sentinel<T> {
    fn new(channel: mpsc::Sender<T>, data: T) -> Sentinel<T> {
        Sentinel {
            value: Some(data),
            supervisor: channel,
            active: true
        }
    }

    fn cancel(mut self) { self.active = false; }
}

#[unsafe_destructor]
impl<T: Send + 'static> Drop for Sentinel<T> {
    fn drop(&mut self) {
        // If we were cancelled, get out of here.
        if !self.active { return; }

        // Respawn ourselves
        let _ = self.supervisor.send(self.value.take().unwrap());
    }
}

