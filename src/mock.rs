use std::io::IoResult;
use std::io::net::ip::SocketAddr;

use net::NetworkStream;

#[deriving(Clone, PartialEq, Show)]
pub struct MockStream;

impl Reader for MockStream {
    fn read(&mut self, _buf: &mut [u8]) -> IoResult<uint> {
        unimplemented!()
    }
}

impl Writer for MockStream {
    fn write(&mut self, _msg: &[u8]) -> IoResult<()> {
        unimplemented!()
    }
}

impl NetworkStream for MockStream {
    fn connect(_host: &str, _port: u16, _scheme: &str) -> IoResult<MockStream> {
        Ok(MockStream)
    }

    fn peer_name(&mut self) -> IoResult<SocketAddr> {
        Ok(from_str("127.0.0.1:1337").unwrap())
    }
}
