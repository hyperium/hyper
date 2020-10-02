use std::sync::Arc;

use hyper::client::conn;
use hyper::rt::Executor as _;
use hyper::{Body, Request};

use crate::io::Io;
use crate::task::{AsTaskType, Exec, Task, TaskType, WeakExec};

pub struct Options {
    builder: conn::Builder,
    /// Use a `Weak` to prevent cycles.
    exec: WeakExec,
}

pub struct ClientConn {
    tx: conn::SendRequest<hyper::Body>,
}

// ===== impl ClientConn =====

ffi_fn! {
    fn hyper_clientconn_handshake(io: *mut Io, options: *mut Options) -> *mut Task {
        if io.is_null() {
            return std::ptr::null_mut();
        }
        if options.is_null() {
            return std::ptr::null_mut();
        }

        let options = unsafe { Box::from_raw(options) };
        let io = unsafe { Box::from_raw(io) };

        Box::into_raw(Task::boxed(async move {
            options.builder.handshake::<_, hyper::Body>(io)
                .await
                .map(|(tx, conn)| {
                    options.exec.execute(Box::pin(async move {
                        let _ = conn.await;
                    }));
                    ClientConn { tx }
                })
        }))
    }
}

ffi_fn! {
    fn hyper_clientconn_send(conn: *mut ClientConn, req: *mut Request<Body>) -> *mut Task {
        if conn.is_null() {
            return std::ptr::null_mut();
        }
        if req.is_null() {
            return std::ptr::null_mut();
        }

        let req = unsafe { Box::from_raw(req) };
        let fut = unsafe { &mut *conn }.tx.send_request(*req);

        Box::into_raw(Task::boxed(fut))
    }
}

ffi_fn! {
    fn hyper_clientconn_free(conn: *mut ClientConn) {
        drop(unsafe { Box::from_raw(conn) });
    }
}

unsafe impl AsTaskType for ClientConn {
    fn as_task_type(&self) -> TaskType {
        TaskType::ClientConn
    }
}

// ===== impl Options =====

ffi_fn! {
    fn hyper_clientconn_options_new() -> *mut Options {
        Box::into_raw(Box::new(Options {
            builder: conn::Builder::new(),
            exec: WeakExec::new(),
        }))
    }
}

ffi_fn! {
    fn hyper_clientconn_options_exec(opts: *mut Options, exec: *const Exec) {
        let opts = unsafe { &mut *opts };

        let exec = unsafe { Arc::from_raw(exec) };
        let weak_exec = Exec::downgrade(&exec);
        std::mem::forget(exec);

        opts.builder.executor(weak_exec.clone());
        opts.exec = weak_exec;
    }
}
