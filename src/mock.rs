use std::fmt;
use std::io::{IoResult, MemReader, MemWriter};
use std::io::net::ip::SocketAddr;

use net::{NetworkStream, NetworkConnector};

pub struct MockStream {
    pub read: MemReader,
    pub write: MemWriter,
}

impl Clone for MockStream {
    fn clone(&self) -> MockStream {
        MockStream {
            read: MemReader::new(self.read.get_ref().to_vec()),
            write: MemWriter::from_vec(self.write.get_ref().to_vec()),
        }
    }
}

impl PartialEq for MockStream {
    fn eq(&self, other: &MockStream) -> bool {
        self.read.get_ref() == other.read.get_ref() &&
            self.write.get_ref() == other.write.get_ref()
    }
}

impl fmt::Show for MockStream {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MockStream {{ read: {}, write: {} }}",
               self.read.get_ref(), self.write.get_ref())
    }

}

impl MockStream {
    pub fn new() -> MockStream {
        MockStream {
            read: MemReader::new(vec![]),
            write: MemWriter::new(),
        }
    }

    pub fn with_input(input: &[u8]) -> MockStream {
        MockStream {
            read: MemReader::new(input.to_vec()),
            write: MemWriter::new(),
        }
    }
}
impl Reader for MockStream {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<uint> {
        self.read.read(buf)
    }
}

impl Writer for MockStream {
    fn write(&mut self, msg: &[u8]) -> IoResult<()> {
        self.write.write(msg)
    }
}

impl NetworkStream for MockStream {
    fn peer_name(&mut self) -> IoResult<SocketAddr> {
        Ok("127.0.0.1:1337".parse().unwrap())
    }
}

pub struct MockConnector;

impl NetworkConnector<MockStream> for MockConnector {
    fn connect(&mut self, _host: &str, _port: u16, _scheme: &str) -> IoResult<MockStream> {
        Ok(MockStream::new())
    }
}

/// new connectors must be created if you wish to intercept requests.
macro_rules! mock_connector (
    ($name:ident {
        $($url:expr => $res:expr)*
    }) => (

        struct $name;

        impl ::net::NetworkConnector<::mock::MockStream> for $name {
            fn connect(&mut self, host: &str, port: u16, scheme: &str) -> ::std::io::IoResult<::mock::MockStream> {
                use std::collections::HashMap;
                debug!("MockStream::connect({}, {}, {})", host, port, scheme);
                let mut map = HashMap::new();
                $(map.insert($url, $res);)*


                let key = format!("{}://{}", scheme, host);
                // ignore port for now
                match map.get(&&*key) {
                    Some(res) => Ok(::mock::MockStream {
                        write: ::std::io::MemWriter::new(),
                        read: ::std::io::MemReader::new(res.to_string().into_bytes())
                    }),
                    None => panic!("{} doesn't know url {}", stringify!($name), key)
                }
            }

        }

    )
);

