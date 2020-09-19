use std::pin::Pin;
use std::task::{Context, Poll};

use libc::size_t;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::task::hyper_waker;

type ReadFn = extern "C" fn(*mut (), *const hyper_waker, *mut u8, size_t) -> size_t;
type WriteFn = extern "C" fn(*mut (), *const hyper_waker, *const u8, size_t) -> size_t;

/// `typedef struct hyper_io hyper_io`
pub struct Io {
    read: ReadFn,
    write: WriteFn,
    userdata: *mut (),
}

ffi_fn! {
    fn hyper_io_new() -> *mut Io {
        Box::into_raw(Box::new(Io {
            read: read_noop,
            write: write_noop,
            userdata: std::ptr::null_mut(),
        }))
    }
}

ffi_fn! {
    fn hyper_io_set_data(io: *mut Io, data: *mut ()) {
        unsafe { &mut *io }.userdata = data;
    }
}

ffi_fn! {
    fn hyper_io_set_read(io: *mut Io, func: ReadFn) {
        unsafe { &mut *io }.read = func;
    }
}

ffi_fn! {
    fn hyper_io_set_write(io: *mut Io, func: WriteFn) {
        unsafe { &mut *io }.write = func;
    }
}

extern "C" fn read_noop(
    _userdata: *mut (),
    _: *const hyper_waker,
    _buf: *mut u8,
    _buf_len: size_t,
) -> size_t {
    0
}

extern "C" fn write_noop(
    _userdata: *mut (),
    _: *const hyper_waker,
    _buf: *const u8,
    _buf_len: size_t,
) -> size_t {
    0
}

impl AsyncRead for Io {
    fn poll_read(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        _buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        todo!("poll_read");
    }
}

impl AsyncWrite for Io {
    fn poll_write(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        _buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        todo!("poll_write");
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

unsafe impl Send for Io {}
unsafe impl Sync for Io {}
