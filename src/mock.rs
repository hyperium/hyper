use std::fmt;
use std::io::{self, Read, Write, Cursor};
use std::net::SocketAddr;
use std::sync::mpsc::Sender;

use net::{NetworkStream, NetworkConnector, ContextVerifier};

pub struct MockStream {
    pub read: Cursor<Vec<u8>>,
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
    pub fn new() -> MockStream {
        MockStream {
            read: Cursor::new(vec![]),
            write: vec![],
        }
    }

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

pub struct MockConnector;

impl NetworkConnector for MockConnector {
    type Stream = MockStream;

    fn connect(&self, _host: &str, _port: u16, _scheme: &str) -> ::Result<MockStream> {
        Ok(MockStream::new())
    }

    fn set_ssl_verifier(&mut self, _verifier: ContextVerifier) {
        // pass
    }
}

/// A mock implementation of the `NetworkConnector` trait that keeps track of all calls to its
/// methods by sending corresponding messages onto a channel.
///
/// Otherwise, it behaves the same as `MockConnector`.
pub struct ChannelMockConnector {
    calls: Sender<String>,
}

impl ChannelMockConnector {
    pub fn new(calls: Sender<String>) -> ChannelMockConnector {
        ChannelMockConnector { calls: calls }
    }
}

impl NetworkConnector for ChannelMockConnector {
    type Stream = MockStream;
    #[inline]
    fn connect(&self, _host: &str, _port: u16, _scheme: &str)
            -> ::Result<MockStream> {
        self.calls.send("connect".into()).unwrap();
        Ok(MockStream::new())
    }

    #[inline]
    fn set_ssl_verifier(&mut self, _verifier: ContextVerifier) {
        self.calls.send("set_ssl_verifier".into()).unwrap();
    }
}

/// new connectors must be created if you wish to intercept requests.
macro_rules! mock_connector (
    ($name:ident {
        $($url:expr => $res:expr)*
    }) => (

        struct $name;

        impl ::net::NetworkConnector for $name {
            type Stream = ::mock::MockStream;
            fn connect(&self, host: &str, port: u16, scheme: &str) -> $crate::Result<::mock::MockStream> {
                use std::collections::HashMap;
                use std::io::Cursor;
                debug!("MockStream::connect({:?}, {:?}, {:?})", host, port, scheme);
                let mut map = HashMap::new();
                $(map.insert($url, $res);)*


                let key = format!("{}://{}", scheme, host);
                // ignore port for now
                match map.get(&*key) {
                    Some(&res) => Ok($crate::mock::MockStream {
                        write: vec![],
                        read: Cursor::new(res.to_owned().into_bytes()),
                    }),
                    None => panic!("{:?} doesn't know url {}", stringify!($name), key)
                }
            }

            fn set_ssl_verifier(&mut self, _verifier: ::net::ContextVerifier) {
                // pass
            }
        }

    )
);

