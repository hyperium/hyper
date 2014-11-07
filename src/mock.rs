use std::io::IoResult;
use std::io::net::ip::{SocketAddr, ToSocketAddr};

use net::{NetworkStream, NetworkConnector};

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

    fn peer_name(&mut self) -> IoResult<SocketAddr> {
        Ok(from_str("127.0.0.1:1337").unwrap())
    }
}

impl NetworkConnector for MockStream {
    fn connect<To: ToSocketAddr>(_addr: To, _scheme: &str) -> IoResult<MockStream> {
        Ok(MockStream)
    }
}
