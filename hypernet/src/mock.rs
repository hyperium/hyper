//! Contains various utility types and macros useful for testing hyper clients.
use std::fmt;
use std::io::{self, Read, Write, Cursor};
use std::net::SocketAddr;

use super::{NetworkStream, NetworkConnector};

/// A `NetworkStream` compatible stream that writes into memory, and reads from memory.
pub struct MockStream {
    /// Data readable from stream.
    pub read: Cursor<Vec<u8>>,
    /// Data written to the stream.
    pub write: Vec<u8>,
}

impl Clone for MockStream {
    fn clone(&self) -> MockStream {
        MockStream {
            read: Cursor::new(self.read.get_ref().clone()),
            write: self.write.clone()
        }
    }
}

impl fmt::Debug for MockStream {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MockStream {{ read: {:?}, write: {:?} }}", self.read.get_ref(), self.write)
    }
}

impl PartialEq for MockStream {
    fn eq(&self, other: &MockStream) -> bool {
        self.read.get_ref() == other.read.get_ref() && self.write == other.write
    }
}

impl MockStream {
    /// Creates a new empty mock stream.
    pub fn new() -> MockStream {
        MockStream {
            read: Cursor::new(vec![]),
            write: vec![],
        }
    }

    /// Creates a new stream with data that can be read from the stream.
    pub fn with_input(input: &[u8]) -> MockStream {
        MockStream {
            read: Cursor::new(input.to_vec()),
            write: vec![]
        }
    }
}

impl Read for MockStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.read.read(buf)
    }
}

impl Write for MockStream {
    fn write(&mut self, msg: &[u8]) -> io::Result<usize> {
        Write::write(&mut self.write, msg)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl NetworkStream for MockStream {
    fn peer_addr(&mut self) -> io::Result<SocketAddr> {
        Ok("127.0.0.1:1337".parse().unwrap())
    }
}

/// A `NetworkConnector` which creates `MockStream` instances exclusively.
/// It may be useful to intercept writes.
pub struct MockConnector;

impl NetworkConnector for MockConnector {
    type Stream = MockStream;

    fn connect(&mut self, _host: &str, _port: u16, _scheme: &str) -> io::Result<MockStream> {
        Ok(MockStream::new())
    }
}

/// This macro maps host URLs to a respective reply, which is given in plain-text.
/// It ignores, but stores, everything that is written to it. However, the stored
/// values are not accessible just yet.
#[macro_export]
macro_rules! mock_connector (
    ($name:ident {
        $($url:expr => $res:expr)*
    }) => (

        struct $name;

        impl ::net::NetworkConnector for $name {
            type Stream = ::mock::MockStream;
            fn connect(&mut self, host: &str, port: u16, scheme: &str) -> ::std::io::Result<::mock::MockStream> {
                use std::collections::HashMap;
                use std::io::Cursor;
                debug!("MockStream::connect({:?}, {:?}, {:?})", host, port, scheme);
                let mut map = HashMap::new();
                $(map.insert($url, $res);)*


                let key = format!("{}://{}", scheme, host);
                // ignore port for now
                match map.get(&*key) {
                    Some(res) => Ok(::mock::MockStream {
                        write: vec![],
                        read: Cursor::new(res.to_string().into_bytes()),
                    }),
                    None => panic!("{:?} doesn't know url {}", stringify!($name), key)
                }
            }

        }

    )
);
