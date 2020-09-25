use std::ffi::c_void;
use std::pin::Pin;
use std::task::{Context, Poll};

use libc::size_t;
use tokio::io::{AsyncRead, AsyncWrite};

/// #define HYPER_IO_PENDING 0xFFFFFFFF
const IO_PENDING: size_t = 0xFFFFFFFF;
/// #define HYPER_IO_ERROR 0xFFFFFFFE
const IO_ERROR: size_t = 0xFFFFFFFE;

type ReadFn = extern "C" fn(*mut c_void, *mut Context<'_>, *mut u8, size_t) -> size_t;
type WriteFn = extern "C" fn(*mut c_void, *mut Context<'_>, *const u8, size_t) -> size_t;

/// `typedef struct hyper_io hyper_io`
pub struct Io {
    read: ReadFn,
    write: WriteFn,
    userdata: *mut c_void,
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
    fn hyper_io_set_data(io: *mut Io, data: *mut c_void) {
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
    _userdata: *mut c_void,
    _: *mut Context<'_>,
    _buf: *mut u8,
    _buf_len: size_t,
) -> size_t {
    0
}

extern "C" fn write_noop(
    _userdata: *mut c_void,
    _: *mut Context<'_>,
    _buf: *const u8,
    _buf_len: size_t,
) -> size_t {
    0
}

impl AsyncRead for Io {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        let buf_ptr = buf.as_mut_ptr();
        let buf_len = buf.len();

        match (self.read)(self.userdata, cx, buf_ptr, buf_len) {
            IO_PENDING => Poll::Pending,
            IO_ERROR => Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::Other, "io error"))),
            ok => {
                Poll::Ready(Ok(ok))
            }
        }
    }
}

impl AsyncWrite for Io {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let buf_ptr = buf.as_ptr();
        let buf_len = buf.len();

        match (self.write)(self.userdata, cx, buf_ptr, buf_len) {
            IO_PENDING => Poll::Pending,
            IO_ERROR => Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::Other, "io error"))),
            ok => Poll::Ready(Ok(ok))
        }
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
