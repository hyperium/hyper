#![feature(never_type)]
#![no_main]
use libfuzzer_sys::fuzz_target;

use std::pin::Pin;

use std::task::{Context, Poll};

use hyper::{Body, Request, Response};

struct WriteVoidReadData(std::io::Cursor<Vec<u8>>);

impl tokio::io::AsyncRead for WriteVoidReadData {
    fn poll_read(
        mut self: core::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        tokio::io::AsyncRead::poll_read(Pin::new(&mut self.0), cx, buf)
    }
}

impl tokio::io::AsyncWrite for WriteVoidReadData {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        Poll::Ready(Ok(buf.len()))
    }
    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Poll::Ready(Ok(()))
    }
}

struct VoidService;

impl hyper::service::Service<Request<Body>> for VoidService {
    type Response = Response<hyper::body::Body>;
    type Error = !;
    type Future = core::future::Ready<Result<Self::Response, !>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _req: Request<Body>) -> Self::Future {
        core::future::ready(Ok(Response::new(hyper::body::Body::empty())))
    }
}

fuzz_target!(|data: Vec<u8>| {
    let s = hyper::server::conn::Http::new();
    let svc = s.serve_connection(WriteVoidReadData(std::io::Cursor::new(data)), VoidService);
    drop(async_io::block_on(svc));
});
