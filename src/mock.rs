use std::fmt;
use std::ascii::AsciiExt;
use std::io::{self, Read, Write, Cursor};
use std::cell::RefCell;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
#[cfg(feature = "timeouts")]
use std::time::Duration;
#[cfg(feature = "timeouts")]
use std::cell::Cell;

use solicit::http::HttpScheme;
use solicit::http::transport::TransportStream;
use solicit::http::frame::{SettingsFrame, Frame};
use solicit::http::connection::{HttpConnection, EndStream, DataChunk};

use header::Headers;
use net::{NetworkStream, NetworkConnector};

#[derive(Clone)]
pub struct MockStream {
    pub read: Cursor<Vec<u8>>,
    pub write: Vec<u8>,
    #[cfg(feature = "timeouts")]
    pub read_timeout: Cell<Option<Duration>>,
    #[cfg(feature = "timeouts")]
    pub write_timeout: Cell<Option<Duration>>
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
        MockStream::with_input(b"")
    }

    #[cfg(not(feature = "timeouts"))]
    pub fn with_input(input: &[u8]) -> MockStream {
        MockStream {
            read: Cursor::new(input.to_vec()),
            write: vec![]
        }
    }

    #[cfg(feature = "timeouts")]
    pub fn with_input(input: &[u8]) -> MockStream {
        MockStream {
            read: Cursor::new(input.to_vec()),
            write: vec![],
            read_timeout: Cell::new(None),
            write_timeout: Cell::new(None),
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

    #[cfg(feature = "timeouts")]
    fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.read_timeout.set(dur);
        Ok(())
    }

    #[cfg(feature = "timeouts")]
    fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.write_timeout.set(dur);
        Ok(())
    }
}

/// A wrapper around a `MockStream` that allows one to clone it and keep an independent copy to the
/// same underlying stream.
#[derive(Clone)]
pub struct CloneableMockStream {
    pub inner: Arc<Mutex<MockStream>>,
}

impl Write for CloneableMockStream {
    fn write(&mut self, msg: &[u8]) -> io::Result<usize> {
        self.inner.lock().unwrap().write(msg)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.lock().unwrap().flush()
    }
}

impl Read for CloneableMockStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.lock().unwrap().read(buf)
    }
}

impl TransportStream for CloneableMockStream {
    fn try_split(&self) -> Result<CloneableMockStream, io::Error> {
        Ok(self.clone())
    }

    fn close(&mut self) -> Result<(), io::Error> {
        Ok(())
    }
}

impl NetworkStream for CloneableMockStream {
    fn peer_addr(&mut self) -> io::Result<SocketAddr> {
        self.inner.lock().unwrap().peer_addr()
    }

    #[cfg(feature = "timeouts")]
    fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.inner.lock().unwrap().set_read_timeout(dur)
    }

    #[cfg(feature = "timeouts")]
    fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.inner.lock().unwrap().set_write_timeout(dur)
    }
}

impl CloneableMockStream {
    pub fn with_stream(stream: MockStream) -> CloneableMockStream {
        CloneableMockStream {
            inner: Arc::new(Mutex::new(stream)),
        }
    }
}

pub struct MockConnector;

impl NetworkConnector for MockConnector {
    type Stream = MockStream;

    fn connect(&self, _host: &str, _port: u16, _scheme: &str) -> ::Result<MockStream> {
        Ok(MockStream::new())
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
            fn connect(&self, host: &str, port: u16, scheme: &str)
                    -> $crate::Result<::mock::MockStream> {
                use std::collections::HashMap;
                debug!("MockStream::connect({:?}, {:?}, {:?})", host, port, scheme);
                let mut map = HashMap::new();
                $(map.insert($url, $res);)*


                let key = format!("{}://{}", scheme, host);
                // ignore port for now
                match map.get(&*key) {
                    Some(&res) => Ok($crate::mock::MockStream::with_input(res.as_bytes())),
                    None => panic!("{:?} doesn't know url {}", stringify!($name), key)
                }
            }
        }

    )
);

impl TransportStream for MockStream {
    fn try_split(&self) -> Result<MockStream, io::Error> {
        Ok(self.clone())
    }

    fn close(&mut self) -> Result<(), io::Error> {
        Ok(())
    }
}

impl MockStream {
    /// Creates a new `MockStream` that will return the response described by the parameters as an
    /// HTTP/2 response. This will also include the correct server preface.
    pub fn new_http2_response(status: &[u8], headers: &Headers, body: Option<Vec<u8>>)
            -> MockStream {
        let resp_bytes = build_http2_response(status, headers, body);
        MockStream::with_input(&resp_bytes)
    }
}

/// Builds up a sequence of bytes that represent a server's response based on the given parameters.
pub fn build_http2_response(status: &[u8], headers: &Headers, body: Option<Vec<u8>>) -> Vec<u8> {
    let mut conn = HttpConnection::new(MockStream::new(), MockStream::new(), HttpScheme::Http);
    // Server preface first
    conn.sender.write(&SettingsFrame::new().serialize()).unwrap();

    let mut resp_headers: Vec<_> = headers.iter().map(|h| {
        (h.name().to_ascii_lowercase().into_bytes(), h.value_string().into_bytes())
    }).collect();
    resp_headers.insert(0, (b":status".to_vec(), status.into()));

    let end = if body.is_none() {
        EndStream::Yes
    } else {
        EndStream::No
    };
    conn.send_headers(resp_headers, 1, end).unwrap();
    if body.is_some() {
        let chunk = DataChunk::new_borrowed(&body.as_ref().unwrap()[..], 1, EndStream::Yes);
        conn.send_data(chunk).unwrap();
    }

    conn.sender.write
}

/// A mock connector that produces `MockStream`s that are set to return HTTP/2 responses.
///
/// This means that the streams' payloads are fairly opaque byte sequences (as HTTP/2 is a binary
/// protocol), which can be understood only be HTTP/2 clients.
pub struct MockHttp2Connector {
    /// The list of streams that the connector returns, in the given order.
    pub streams: RefCell<Vec<CloneableMockStream>>,
}

impl MockHttp2Connector {
    /// Creates a new `MockHttp2Connector` with no streams.
    pub fn new() -> MockHttp2Connector {
        MockHttp2Connector {
            streams: RefCell::new(Vec::new()),
        }
    }

    /// Adds a new `CloneableMockStream` to the end of the connector's stream queue.
    ///
    /// Streams are returned in a FIFO manner.
    pub fn add_stream(&mut self, stream: CloneableMockStream) {
        self.streams.borrow_mut().push(stream);
    }

    /// Adds a new response stream that will be placed to the end of the connector's stream queue.
    ///
    /// Returns a separate `CloneableMockStream` that allows the user to inspect what is written
    /// into the original stream.
    pub fn new_response_stream(&mut self, status: &[u8], headers: &Headers, body: Option<Vec<u8>>)
            -> CloneableMockStream {
        let stream = MockStream::new_http2_response(status, headers, body);
        let stream = CloneableMockStream::with_stream(stream);
        let ret = stream.clone();
        self.add_stream(stream);

        ret
    }
}

impl NetworkConnector for MockHttp2Connector {
    type Stream = CloneableMockStream;
    #[inline]
    fn connect(&self, _host: &str, _port: u16, _scheme: &str)
            -> ::Result<CloneableMockStream> {
        Ok(self.streams.borrow_mut().remove(0))
    }
}
