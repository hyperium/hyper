#![deny(warnings)]
extern crate hyper;
#[macro_use]
extern crate log;
extern crate pretty_env_logger;
extern crate tokio;

use hyper::{Body, Response, Server};
use hyper::service::service_fn_ok;
use hyper::rt::{self, Future};

fn main() {
    pretty_env_logger::init();

    let addr = ([127, 0, 0, 1], 3000).into();

    let server = Server::bind(&addr)
        .acceptor(acceptor::accept)
        .serve(|| {
            service_fn_ok(|_| {
                Response::new(Body::from("Hello World!"))
            })
        })
        .map_err(|e| eprintln!("server error: {}", e));

    println!("Listening on http://{}", addr);

    rt::run(server);
}

mod acceptor {
    use std::io::{self, Read, Write};
    use tokio::io::{AsyncRead, AsyncWrite};
    use tokio::prelude::Poll;

    pub struct WrappedIO<T>(T);

    impl<T: Read> Read for WrappedIO<T> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            trace!("WrappedIO::read");
            self.0.read(buf)
        }
    }

    impl<T: Write> Write for WrappedIO<T> {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            trace!("WrappedIO::write");
            self.0.write(buf)
        }

        fn flush(&mut self) -> io::Result<()> {
            self.0.flush()
        }
    }

    impl<T: AsyncRead> AsyncRead for WrappedIO<T> {
    }

    impl<T: AsyncWrite> AsyncWrite for WrappedIO<T> {
        fn shutdown(&mut self) -> Poll<(), io::Error> {
            self.0.shutdown()
        }
    }

    pub fn accept<T>(io: T) -> io::Result<WrappedIO<T>>
    where
        T: AsyncRead + AsyncWrite,
    {
        Ok(WrappedIO(io))
    }
}
