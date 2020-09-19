use hyper::client::conn;
use hyper::{Body, Request};

use crate::io::Io;
use crate::task::Task;

pub struct Options {
    builder: conn::Builder,
}

pub struct ClientConn {
    tx: conn::SendRequest<hyper::Body>,
}

ffi_fn! {
    fn hyper_clientconn_handshake(io: *mut Io, options: *mut Options) -> *mut Task {
        let options = unsafe { Box::from_raw(options) };
        let io = unsafe { Box::from_raw(io) };

        Box::into_raw(Task::boxed(options.builder.handshake::<_, hyper::Body>(io)))
    }
}

ffi_fn! {
    fn hyper_clientconn_send(conn: *mut ClientConn, req: *mut Request<Body>) -> *mut Task {
        let req = unsafe { Box::from_raw(req) };
        let fut = unsafe { &mut *conn }.tx.send_request(*req);

        Box::into_raw(Task::boxed(fut))
    }
}
