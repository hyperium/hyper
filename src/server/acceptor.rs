use std::thread::{Thread, JoinGuard};
use std::sync::Arc;
use std::sync::mpsc;
use net::NetworkAcceptor;

pub struct AcceptorPool<A: NetworkAcceptor> {
    acceptor: A
}

impl<A: NetworkAcceptor> AcceptorPool<A> {
    /// Create a thread pool to manage the acceptor.
    pub fn new(acceptor: A) -> AcceptorPool<A> {
        AcceptorPool { acceptor: acceptor }
    }

    /// Runs the acceptor pool. Blocks until the acceptors are closed.
    ///
    /// ## Panics
    ///
    /// Panics if threads == 0.
    pub fn accept<F: Fn(A::Stream) + Send + Sync>(self,
                                                  work: F,
                                                  threads: usize) -> JoinGuard<'static, ()> {
        assert!(threads != 0, "Can't accept on 0 threads.");

        // Replace with &F when Send changes land.
        let work = Arc::new(work);

        let (super_tx, supervisor_rx) = mpsc::channel();

        let spawn =
            move || spawn_with(super_tx.clone(), work.clone(), self.acceptor.clone());

        // Go
        for _ in 0..threads { spawn() }

        // Spawn the supervisor
        Thread::scoped(move || for () in supervisor_rx.iter() { spawn() })
    }
}

fn spawn_with<A, F>(supervisor: mpsc::Sender<()>, work: Arc<F>, mut acceptor: A)
where A: NetworkAcceptor,
      F: Fn(<A as NetworkAcceptor>::Stream) + Send + Sync {
    use std::old_io::EndOfFile;

    Thread::spawn(move || {
        let sentinel = Sentinel::new(supervisor, ());

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
    });
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
impl<T: Send> Drop for Sentinel<T> {
    fn drop(&mut self) {
        // If we were cancelled, get out of here.
        if !self.active { return; }

        // Respawn ourselves
        let _ = self.supervisor.send(self.value.take().unwrap());
    }
}

