use std::borrow::Cow;
use std::io;
use std::net::{SocketAddr, Shutdown};
use std::time::Duration;

use client::scheme::Scheme;
use method::Method;
use net::{NetworkConnector, HttpConnector, NetworkStream, SslClient};

pub fn tunnel(proxy: (Scheme, Cow<'static, str>, u16)) -> Proxy<HttpConnector, self::no_ssl::Plaintext> {
    Proxy {
        connector: HttpConnector,
        proxy: proxy,
        ssl: self::no_ssl::Plaintext,
    }

}

pub struct Proxy<C, S>
where C: NetworkConnector + Send + Sync + 'static,
      C::Stream: NetworkStream + Send + Clone,
      S: SslClient<C::Stream> {
    pub connector: C,
    pub proxy: (Scheme, Cow<'static, str>, u16),
    pub ssl: S,
}

impl<C, S> NetworkConnector for Proxy<C, S>
where C: NetworkConnector + Send + Sync + 'static,
      C::Stream: NetworkStream + Send + Clone,
      S: SslClient<C::Stream> {
    type Stream = Proxied<C::Stream, S::Stream>;

    fn connect(&self, host: &str, port: u16, scheme: &str) -> ::Result<Self::Stream> {
        use httparse;
        use std::io::{Read, Write};
        use ::version::HttpVersion::Http11;
        trace!("{:?} proxy for '{}://{}:{}'", self.proxy, scheme, host, port);
        match scheme {
            "http" => {
                self.connector.connect(self.proxy.1.as_ref(), self.proxy.2, self.proxy.0.as_ref())
                    .map(Proxied::Normal)
            },
            "https" => {
                let mut stream = try!(self.connector.connect(self.proxy.1.as_ref(), self.proxy.2, self.proxy.0.as_ref()));
                trace!("{:?} CONNECT {}:{}", self.proxy, host, port);
                try!(write!(&mut stream, "{method} {host}:{port} {version}\r\nHost: {host}:{port}\r\n\r\n",
                            method=Method::Connect, host=host, port=port, version=Http11));
                try!(stream.flush());
                let mut buf = [0; 1024];
                let mut n = 0;
                while n < buf.len() {
                    n += try!(stream.read(&mut buf[n..]));
                    let mut headers = [httparse::EMPTY_HEADER; 10];
                    let mut res = httparse::Response::new(&mut headers);
                    if try!(res.parse(&buf[..n])).is_complete() {
                        let code = res.code.expect("complete parsing lost code");
                        if code >= 200 && code < 300 {
                            trace!("CONNECT success = {:?}", code);
                            return self.ssl.wrap_client(stream, host)
                                .map(Proxied::Tunneled)
                        } else {
                            trace!("CONNECT response = {:?}", code);
                            return Err(::Error::Status);
                        }
                    }
                }
                Err(::Error::TooLarge)
            },
            _ => Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid scheme").into())
        }
    }
}

#[derive(Debug)]
pub enum Proxied<T1, T2> {
    Normal(T1),
    Tunneled(T2)
}

#[cfg(test)]
impl<T1, T2> Proxied<T1, T2> {
    pub fn into_normal(self) -> Result<T1, Self> {
        match self {
            Proxied::Normal(t1) => Ok(t1),
            _ => Err(self)
        }
    }

    pub fn into_tunneled(self) -> Result<T2, Self> {
        match self {
            Proxied::Tunneled(t2) => Ok(t2),
            _ => Err(self)
        }
    }
}

impl<T1: NetworkStream, T2: NetworkStream> io::Read for Proxied<T1, T2> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match *self {
            Proxied::Normal(ref mut t) => io::Read::read(t, buf),
            Proxied::Tunneled(ref mut t) => io::Read::read(t, buf),
        }
    }
}

impl<T1: NetworkStream, T2: NetworkStream> io::Write for Proxied<T1, T2> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match *self {
            Proxied::Normal(ref mut t) => io::Write::write(t, buf),
            Proxied::Tunneled(ref mut t) => io::Write::write(t, buf),
        }
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        match *self {
            Proxied::Normal(ref mut t) => io::Write::flush(t),
            Proxied::Tunneled(ref mut t) => io::Write::flush(t),
        }
    }
}

impl<T1: NetworkStream, T2: NetworkStream> NetworkStream for Proxied<T1, T2> {
    #[inline]
    fn peer_addr(&mut self) -> io::Result<SocketAddr> {
        match *self {
            Proxied::Normal(ref mut s) => s.peer_addr(),
            Proxied::Tunneled(ref mut s) => s.peer_addr()
        }
    }

    #[inline]
    fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        match *self {
            Proxied::Normal(ref inner) => inner.set_read_timeout(dur),
            Proxied::Tunneled(ref inner) => inner.set_read_timeout(dur)
        }
    }

    #[inline]
    fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        match *self {
            Proxied::Normal(ref inner) => inner.set_write_timeout(dur),
            Proxied::Tunneled(ref inner) => inner.set_write_timeout(dur)
        }
    }

    #[inline]
    fn close(&mut self, how: Shutdown) -> io::Result<()> {
        match *self {
            Proxied::Normal(ref mut s) => s.close(how),
            Proxied::Tunneled(ref mut s) => s.close(how)
        }
    }
}

#[cfg(not(any(feature = "openssl", feature = "security-framework")))]
mod no_ssl {
    use std::io;
    use std::net::{Shutdown, SocketAddr};
    use std::time::Duration;

    use net::{SslClient, NetworkStream};

    pub struct Plaintext;

    #[derive(Clone)]
    pub enum Void {}

    impl io::Read for Void {
        #[inline]
        fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
            match *self {}
        }
    }

    impl io::Write for Void {
        #[inline]
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            match *self {}
        }

        #[inline]
        fn flush(&mut self) -> io::Result<()> {
            match *self {}
        }
    }

    impl NetworkStream for Void {
        #[inline]
        fn peer_addr(&mut self) -> io::Result<SocketAddr> {
            match *self {}
        }

        #[inline]
        fn set_read_timeout(&self, _dur: Option<Duration>) -> io::Result<()> {
            match *self {}
        }

        #[inline]
        fn set_write_timeout(&self, _dur: Option<Duration>) -> io::Result<()> {
            match *self {}
        }

        #[inline]
        fn close(&mut self, _how: Shutdown) -> io::Result<()> {
            match *self {}
        }
    }

    impl<T: NetworkStream + Send + Clone> SslClient<T> for Plaintext {
        type Stream = Void;

        fn wrap_client(&self, _stream: T, _host: &str) -> ::Result<Self::Stream> {
            Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid scheme").into())
        }
    }
}
