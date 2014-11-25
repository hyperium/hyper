use std::fmt;
use std::io::{IoResult, MemReader, MemWriter};
use std::io::net::ip::{SocketAddr, ToSocketAddr};

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
        Ok(from_str("127.0.0.1:1337").unwrap())
    }
}

impl NetworkConnector for MockStream {
    fn connect<To: ToSocketAddr>(_addr: To, _scheme: &str) -> IoResult<MockStream> {
        Ok(MockStream::new())
    }
}
