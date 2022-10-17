use std::sync::Arc;
use std::ptr;
use std::ffi::c_void;

use crate::ffi::UserDataPointer;
use crate::ffi::io::hyper_io;
use crate::ffi::http_types::{hyper_request, hyper_response};
use crate::ffi::task::{hyper_executor, hyper_task, hyper_task_return_type, AsTaskType, IntoDynTaskType, WeakExec};
use crate::server::conn::{Connection, Http};

pub struct hyper_serverconn_options(Http<WeakExec>);
pub struct hyper_serverconn(Connection<hyper_io, hyper_service, WeakExec>);
pub struct hyper_service {
    service_fn: hyper_service_callback,
    userdata: UserDataPointer,
}
pub struct hyper_response_channel(futures_channel::oneshot::Sender<Box<hyper_response>>);

type hyper_service_callback = extern "C" fn(*mut c_void, *mut hyper_request, *mut hyper_response, *mut hyper_response_channel);

ffi_fn! {
    fn hyper_serverconn_options_new(exec: *const hyper_executor) -> *mut hyper_serverconn_options {
        let exec = non_null! { Arc::from_raw(exec) ?= ptr::null_mut() };
        let weak_exec = hyper_executor::downgrade(&exec);
        std::mem::forget(exec); // We've not incremented the strong count when we loaded
                                // `from_raw`
        Box::into_raw(Box::new(hyper_serverconn_options(Http::new().with_executor(weak_exec))))
    }
}

ffi_fn! {
    fn hyper_service_new(service_fn: hyper_service_callback) -> *mut hyper_service {
        Box::into_raw(Box::new(hyper_service {
            service_fn: service_fn,
            userdata: UserDataPointer(ptr::null_mut()),
        }))
    } ?= ptr::null_mut()
}

ffi_fn! {
    fn hyper_service_set_userdata(service: *mut hyper_service, userdata: *mut c_void){
        let s = non_null!{ &mut *service ?= () };
        s.userdata = UserDataPointer(userdata);
    }
}

ffi_fn! {
    fn hyper_serve_connection(serverconn_options: *mut hyper_serverconn_options, io: *mut hyper_io, service: *mut hyper_service) -> *mut hyper_task {
        let serverconn_options = non_null! { &mut *serverconn_options ?= ptr::null_mut() };
        let io = non_null! { Box::from_raw(io) ?= ptr::null_mut() };
        let service = non_null! { Box::from_raw(service) ?= ptr::null_mut() };
        let task = hyper_task::boxed(hyper_serverconn(serverconn_options.0.serve_connection(*io, *service)));
        Box::into_raw(task)
    } ?= ptr::null_mut()
}

ffi_fn! {
    fn hyper_response_channel_send(channel: *mut hyper_response_channel, response: *mut hyper_response) {
        let channel = non_null! { Box::from_raw(channel) ?= () };
        let response = non_null! { Box::from_raw(response) ?= () };
        let _ = channel.0.send(response);
    }
}

impl crate::service::Service<crate::Request<crate::body::Recv>> for hyper_service {
    type Response = crate::Response<crate::body::Recv>;
    type Error = crate::Error;
    type Future = std::pin::Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&mut self, req: crate::Request<crate::body::Recv>) -> Self::Future {
        let req_ptr = Box::into_raw(Box::new(hyper_request(req)));
        let res = crate::Response::new(crate::body::Recv::empty());
        let res_ptr = Box::into_raw(Box::new(hyper_response(res)));

        let (tx, rx) = futures_channel::oneshot::channel();
        let res_channel = Box::into_raw(Box::new(hyper_response_channel(tx)));

        (self.service_fn)(self.userdata.0, req_ptr, res_ptr, res_channel);

        Box::pin(async move { 
            let res = rx.await.expect("Channel closed?");
            Ok(res.0)
        })
    }
}

impl std::future::Future for hyper_serverconn {
    type Output = crate::Result<()>;

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        std::pin::Pin::new(&mut self.0).poll(cx)
    }
}
