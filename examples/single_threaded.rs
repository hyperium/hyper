#![deny(warnings)]

use http_body_util::BodyExt;
use hyper::server::conn::http2;
use std::cell::Cell;
use std::net::SocketAddr;
use std::rc::Rc;
use tokio::io::{self, AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpListener;

use hyper::body::{Body as HttpBody, Bytes, Frame};
use hyper::service::service_fn;
use hyper::Request;
use hyper::{Error, Response};
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::thread;
use tokio::net::TcpStream;

struct Body {
    // Our Body type is !Send and !Sync:
    _marker: PhantomData<*const ()>,
    data: Option<Bytes>,
}

impl From<String> for Body {
    fn from(a: String) -> Self {
        Body {
            _marker: PhantomData,
            data: Some(a.into()),
        }
    }
}

impl HttpBody for Body {
    type Data = Bytes;
    type Error = Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        Poll::Ready(self.get_mut().data.take().map(|d| Ok(Frame::data(d))))
    }
}

fn main() {
    pretty_env_logger::init();

    let server = thread::spawn(move || {
        // Configure a runtime for the server that runs everything on the current thread
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build runtime");

        // Combine it with a `LocalSet,  which means it can spawn !Send futures...
        let local = tokio::task::LocalSet::new();
        local.block_on(&rt, server()).unwrap();
    });

    let client = thread::spawn(move || {
        // Configure a runtime for the client that runs everything on the current thread
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build runtime");

        // Combine it with a `LocalSet,  which means it can spawn !Send futures...
        let local = tokio::task::LocalSet::new();
        local
            .block_on(
                &rt,
                client("http://localhost:3000".parse::<hyper::Uri>().unwrap()),
            )
            .unwrap();
    });

    server.join().unwrap();
    client.join().unwrap();
}

async fn server() -> Result<(), Box<dyn std::error::Error>> {
    let mut stdout = io::stdout();

    let addr: SocketAddr = ([127, 0, 0, 1], 3000).into();
    // Using a !Send request counter is fine on 1 thread...
    let counter = Rc::new(Cell::new(0));

    let listener = TcpListener::bind(addr).await?;

    stdout
        .write_all(format!("Listening on http://{}", addr).as_bytes())
        .await
        .unwrap();
    stdout.flush().await.unwrap();

    loop {
        let (stream, _) = listener.accept().await?;

        // For each connection, clone the counter to use in our service...
        let cnt = counter.clone();

        let service = service_fn(move |_| {
            let prev = cnt.get();
            cnt.set(prev + 1);
            let value = cnt.get();
            async move { Ok::<_, Error>(Response::new(Body::from(format!("Request #{}", value)))) }
        });

        tokio::task::spawn_local(async move {
            if let Err(err) = http2::Builder::new(LocalExec)
                .serve_connection(stream, service)
                .await
            {
                let mut stdout = io::stdout();
                stdout
                    .write_all(format!("Error serving connection: {:?}", err).as_bytes())
                    .await
                    .unwrap();
                stdout.flush().await.unwrap();
            }
        });
    }
}

struct IOTypeNotSend {
    _marker: PhantomData<*const ()>,
    stream: TcpStream,
}

impl IOTypeNotSend {
    fn new(stream: TcpStream) -> Self {
        Self {
            _marker: PhantomData,
            stream,
        }
    }
}

impl AsyncWrite for IOTypeNotSend {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.stream).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

impl AsyncRead for IOTypeNotSend {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}

async fn client(url: hyper::Uri) -> Result<(), Box<dyn std::error::Error>> {
    let host = url.host().expect("uri has no host");
    let port = url.port_u16().unwrap_or(80);
    let addr = format!("{}:{}", host, port);
    let stream = TcpStream::connect(addr).await?;

    let stream = IOTypeNotSend::new(stream);

    let (mut sender, conn) = hyper::client::conn::http2::handshake(LocalExec, stream).await?;

    tokio::task::spawn_local(async move {
        if let Err(err) = conn.await {
            let mut stdout = io::stdout();
            stdout
                .write_all(format!("Connection failed: {:?}", err).as_bytes())
                .await
                .unwrap();
            stdout.flush().await.unwrap();
        }
    });

    let authority = url.authority().unwrap().clone();

    // Make 4 requests
    for _ in 0..4 {
        let req = Request::builder()
            .uri(url.clone())
            .header(hyper::header::HOST, authority.as_str())
            .body(Body::from("test".to_string()))?;

        let mut res = sender.send_request(req).await?;

        let mut stdout = io::stdout();
        stdout
            .write_all(format!("Response: {}\n", res.status()).as_bytes())
            .await
            .unwrap();
        stdout
            .write_all(format!("Headers: {:#?}\n", res.headers()).as_bytes())
            .await
            .unwrap();
        stdout.flush().await.unwrap();

        // Print the response body
        while let Some(next) = res.frame().await {
            let frame = next?;
            if let Some(chunk) = frame.data_ref() {
                stdout.write_all(&chunk).await.unwrap();
            }
        }
        stdout.write_all(b"\n-----------------\n").await.unwrap();
        stdout.flush().await.unwrap();
    }
    Ok(())
}

// NOTE: This part is only needed for HTTP/2. HTTP/1 doesn't need an executor.
//
// Since the Server needs to spawn some background tasks, we needed
// to configure an Executor that can spawn !Send futures...
#[derive(Clone, Copy, Debug)]
struct LocalExec;

impl<F> hyper::rt::Executor<F> for LocalExec
where
    F: std::future::Future + 'static, // not requiring `Send`
{
    fn execute(&self, fut: F) {
        // This will spawn into the currently running `LocalSet`.
        tokio::task::spawn_local(fut);
    }
}
