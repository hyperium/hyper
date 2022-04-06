use http::{Request, Response};
use std::future::Future;
use std::pin::Pin;
use std::ptr;
use std::sync::Arc;
use std::task::{Context, Poll};

use libc::{c_int, c_uint, c_void, size_t};

use crate::body::Body;
use crate::common::exec::ConnStreamExec;
use crate::server::conn::{Connection, Http};
use crate::service::Service;

use super::error::hyper_code;
use super::http_types::{hyper_request, hyper_response};
use super::io::hyper_io;
use super::task::{hyper_executor, hyper_task, WeakExec};

pub struct hyper_http(Http<WeakExec>);

// Runtime-related functions are not wrapped.

ffi_fn! {
    fn hyper_http_new(exec: *mut hyper_executor) -> *mut hyper_http {
        let exec = non_null! { Arc::from_raw(exec) ?= ptr::null_mut() };
        let weak_exec = hyper_executor::downgrade(&exec);
        std::mem::forget(exec);
        Box::into_raw(Box::new(hyper_http(Http::new().with_executor(weak_exec))))
    }
}

ffi_fn! {
    fn hyper_http_http1_only(http: *mut hyper_http, enabled: c_int) -> hyper_code {
        let http = non_null! { &mut *http ?= hyper_code::HYPERE_INVALID_ARG };
        http.0.http1_only(enabled != 0);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    fn hyper_http_http1_half_close(http: *mut hyper_http, enabled: c_int) -> hyper_code {
        let http = non_null! { &mut *http ?= hyper_code::HYPERE_INVALID_ARG };
        http.0.http1_half_close(enabled != 0);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    fn hyper_http_http1_keep_alive(http: *mut hyper_http, enabled: c_int) -> hyper_code {
        let http = non_null! { &mut *http ?= hyper_code::HYPERE_INVALID_ARG };
        http.0.http1_keep_alive(enabled != 0);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    fn hyper_http_http1_title_case_headers(http: *mut hyper_http, enabled: c_int) -> hyper_code {
        let http = non_null! { &mut *http ?= hyper_code::HYPERE_INVALID_ARG };
        http.0.http1_title_case_headers(enabled != 0);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    fn hyper_http_http1_preserve_header_case(http: *mut hyper_http, enabled: c_int) -> hyper_code {
        let http = non_null! { &mut *http ?= hyper_code::HYPERE_INVALID_ARG };
        http.0.http1_preserve_header_case(enabled != 0);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    fn hyper_http_http1_writev(http: *mut hyper_http, enabled: c_int) -> hyper_code {
        let http = non_null! { &mut *http ?= hyper_code::HYPERE_INVALID_ARG };
        http.0.http1_writev(enabled != 0);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    fn hyper_http_http2_only(http: *mut hyper_http, enabled: c_int) -> hyper_code {
        let http = non_null! { &mut *http ?= hyper_code::HYPERE_INVALID_ARG };
        http.0.http2_only(enabled != 0);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    fn hyper_http_http2_initial_stream_window_size(http: *mut hyper_http, window_size: c_uint) -> hyper_code {
        let http = non_null! { &mut *http ?= hyper_code::HYPERE_INVALID_ARG };
        http.0.http2_initial_stream_window_size(if window_size == 0 { None } else { Some(window_size) });
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    fn hyper_http_http2_initial_connection_window_size(http: *mut hyper_http, window_size: c_uint) -> hyper_code {
        let http = non_null! { &mut *http ?= hyper_code::HYPERE_INVALID_ARG };
        http.0.http2_initial_connection_window_size(if window_size == 0 { None } else { Some(window_size) });
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    fn hyper_http_http2_adaptive_window(http: *mut hyper_http, enabled: c_int) -> hyper_code {
        let http = non_null! { &mut *http ?= hyper_code::HYPERE_INVALID_ARG };
        http.0.http2_adaptive_window(enabled != 0);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    fn hyper_http_http2_max_frame_size(http: *mut hyper_http, frame_size: c_uint) -> hyper_code {
        let http = non_null! { &mut *http ?= hyper_code::HYPERE_INVALID_ARG };
        http.0.http2_max_frame_size(if frame_size == 0 { None } else { Some(frame_size) });
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    fn hyper_http_http2_max_concurrent_streams(http: *mut hyper_http, max_streams: c_uint) -> hyper_code {
        let http = non_null! { &mut *http ?= hyper_code::HYPERE_INVALID_ARG };
        http.0.http2_max_concurrent_streams(if max_streams == 0 { None } else { Some(max_streams) });
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    fn hyper_http_http2_max_send_buf_size(http: *mut hyper_http, max_buf_size: usize) -> hyper_code {
        let http = non_null! { &mut *http ?= hyper_code::HYPERE_INVALID_ARG };
        http.0.http2_max_send_buf_size(max_buf_size);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    fn hyper_http_http2_enable_connect_protocol(http: *mut hyper_http) -> hyper_code {
        let http = non_null! { &mut *http ?= hyper_code::HYPERE_INVALID_ARG };
        http.0.http2_enable_connect_protocol();
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    fn hyper_http_max_buf_size(http: *mut hyper_http, max_buf_size: usize) -> hyper_code {
        let http = non_null! { &mut *http ?= hyper_code::HYPERE_INVALID_ARG };
        http.0.max_buf_size(max_buf_size);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    fn hyper_http_pipeline_flush(http: *mut hyper_http, enabled: c_int) -> hyper_code {
        let http = non_null! { &mut *http ?= hyper_code::HYPERE_INVALID_ARG };
        http.0.pipeline_flush(enabled != 0);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    fn hyper_http_serve_connection(http: *mut hyper_http, io: *mut hyper_io, service: *mut hyper_service) -> *mut hyper_serverconn {
        let http = non_null! { &mut *http ?= ptr::null_mut() };
        let io = non_null! { Box::from_raw(io) ?= ptr::null_mut() };
        let service = non_null! { Box::from_raw(service) ?= ptr::null_mut() };

        Box::into_raw(Box::new(hyper_serverconn(http.0.serve_connection(*io, *service))))
    } ?= ptr::null_mut()
}

pub struct hyper_service {
    service_func: hyper_service_callback,
    userdata: *mut c_void,
}

type hyper_service_callback =
    extern "C" fn(*mut c_void, *mut hyper_request, *mut hyper_response) -> ();

ffi_fn! {
    fn hyper_service_new() -> *mut hyper_service {
        Box::into_raw(Box::new(hyper_service {
            service_func: service_noop,
            userdata: ptr::null_mut(),
        }))
    } ?= ptr::null_mut()
}

ffi_fn! {
    fn hyper_service_set_func(service: *mut hyper_service, func: hyper_service_callback){
        let s = non_null!{ &mut *service ?= () };
        s.service_func = func;
    }
}

ffi_fn! {
    fn hyper_service_set_userdata(service: *mut hyper_service, userdata: *mut c_void){
        let s = non_null!{ &mut *service ?= () };
        s.userdata = userdata;
    }
}

impl Service<Request<Body>> for hyper_service {
    type Response = Response<Body>;
    type Error = crate::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let req_ptr = Box::into_raw(Box::new(hyper_request(req)));
        let res = Response::new(Body::empty());
        let res_ptr = Box::into_raw(Box::new(hyper_response(res)));

        (self.service_func)(self.userdata, req_ptr, res_ptr);

        let hyper_res = non_null! {
            Box::from_raw(res_ptr) ?= Box::pin(async { Err(crate::error::Error::new(
                crate::error::Kind::Io
            ))})
        };

        Box::pin(async move { Ok((*hyper_res).0) })
    }
}

/// cbindgen:ignore
extern "C" fn service_noop(
    _userdata: *mut c_void,
    _req: *mut hyper_request,
    _res: *mut hyper_response,
) {
}

pub struct hyper_serverconn(Connection<hyper_io, hyper_service, WeakExec>);
