use std::io;
use std::net::{IpAddr, ToSocketAddrs};
use std::thread;

use ::spmc;

use http::channel;

pub struct Dns {
    tx: spmc::Sender<String>,
    rx: channel::Receiver<Answer>,
}

pub type Answer = (String, io::Result<IpAddr>);

impl Dns {
    pub fn new(notify: (channel::Sender<Answer>, channel::Receiver<Answer>), threads: usize) -> Dns {
        let (tx, rx) = spmc::channel();
        for _ in 0..threads {
            work(rx.clone(), notify.0.clone());
        }
        Dns {
            tx: tx,
            rx: notify.1,
        }
    }

    pub fn resolve<T: Into<String>>(&self, hostname: T) {
        self.tx.send(hostname.into()).expect("Workers all died horribly");
    }

    pub fn resolved(&self) -> Result<Answer, channel::TryRecvError> {
        self.rx.try_recv()
    }
}

fn work(rx: spmc::Receiver<String>, notify: channel::Sender<Answer>) {
    thread::spawn(move || {
        let mut worker = Worker::new(rx, notify);
        let rx = worker.rx.as_ref().expect("Worker lost rx");
        let notify = worker.notify.as_ref().expect("Worker lost notify");
        while let Ok(host) = rx.recv() {
            debug!("resolve {:?}", host);
            let res = match (&*host, 80).to_socket_addrs().map(|mut i| i.next()) {
                Ok(Some(addr)) => (host, Ok(addr.ip())),
                Ok(None) => (host, Err(io::Error::new(io::ErrorKind::Other, "no addresses found"))),
                Err(e) => (host, Err(e))
            };

            if let Err(_) = notify.send(res) {
                break;
            }
        }
        worker.shutdown = true;
    });
}

struct Worker {
    rx: Option<spmc::Receiver<String>>,
    notify: Option<channel::Sender<Answer>>,
    shutdown: bool,
}

impl Worker {
    fn new(rx: spmc::Receiver<String>, notify: channel::Sender<Answer>) -> Worker {
        Worker {
            rx: Some(rx),
            notify: Some(notify),
            shutdown: false,
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        if !self.shutdown {
            trace!("Worker.drop panicked, restarting");
            work(self.rx.take().expect("Worker lost rx"),
                self.notify.take().expect("Worker lost notify"));
        } else {
            trace!("Worker.drop shutdown, closing");
        }
    }
}
