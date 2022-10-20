use std::sync::Arc;
use std::ptr;
use std::ffi::{c_void, c_uint};

use crate::ffi::UserDataPointer;
use crate::ffi::io::hyper_io;
use crate::ffi::error::hyper_code;
use crate::ffi::http_types::{hyper_request, hyper_response};
use crate::ffi::task::{hyper_executor, hyper_task, WeakExec};
use crate::server::conn::Http;

pub struct hyper_serverconn_options(Http<WeakExec>);
pub struct hyper_service {
    service_fn: hyper_service_callback,
    userdata: UserDataPointer,
}
pub struct hyper_response_channel(futures_channel::oneshot::Sender<Box<hyper_response>>);

type hyper_service_callback = extern "C" fn(*mut c_void, *mut hyper_request, *mut hyper_response_channel);

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
    fn hyper_serverconn_options_free(opts: *mut hyper_serverconn_options) {
        let _ = non_null! { Box::from_raw(opts) ?= () };
    }
}

ffi_fn! {
    fn hyper_serverconn_options_http1_only(opts: *mut hyper_serverconn_options, enabled: bool) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.http1_only(enabled);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    fn hyper_serverconn_options_http1_half_close(opts: *mut hyper_serverconn_options, enabled: bool) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.http1_half_close(enabled);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    fn hyper_serverconn_options_http1_keep_alive(opts: *mut hyper_serverconn_options, enabled: bool) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.http1_keep_alive(enabled);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    fn hyper_serverconn_options_http1_title_case_headers(opts: *mut hyper_serverconn_options, enabled: bool) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.http1_title_case_headers(enabled);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    fn hyper_serverconn_options_http1_preserve_header_case(opts: *mut hyper_serverconn_options, enabled: bool) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.http1_preserve_header_case(enabled);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    fn hyper_serverconn_options_http1_writev(opts: *mut hyper_serverconn_options, enabled: bool) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.http1_writev(enabled);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    fn hyper_serverconn_options_http2_only(opts: *mut hyper_serverconn_options, enabled: bool) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.http2_only(enabled);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    fn hyper_serverconn_options_http2_initial_stream_window_size(opts: *mut hyper_serverconn_options, window_size: c_uint) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.http2_initial_stream_window_size(if window_size == 0 { None } else { Some(window_size) });
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    fn hyper_serverconn_options_http2_initial_connection_window_size(opts: *mut hyper_serverconn_options, window_size: c_uint) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.http2_initial_connection_window_size(if window_size == 0 { None } else { Some(window_size) });
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    fn hyper_serverconn_options_http2_adaptive_window(opts: *mut hyper_serverconn_options, enabled: bool) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.http2_adaptive_window(enabled);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    fn hyper_serverconn_options_http2_max_frame_size(opts: *mut hyper_serverconn_options, frame_size: c_uint) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.http2_max_frame_size(if frame_size == 0 { None } else { Some(frame_size) });
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    fn hyper_serverconn_options_http2_max_concurrent_streams(opts: *mut hyper_serverconn_options, max_streams: c_uint) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.http2_max_concurrent_streams(if max_streams == 0 { None } else { Some(max_streams) });
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    fn hyper_serverconn_options_http2_max_send_buf_size(opts: *mut hyper_serverconn_options, max_buf_size: usize) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.http2_max_send_buf_size(max_buf_size);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    fn hyper_serverconn_options_http2_enable_connect_protocol(opts: *mut hyper_serverconn_options) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.http2_enable_connect_protocol();
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    fn hyper_serverconn_options_max_buf_size(opts: *mut hyper_serverconn_options, max_buf_size: usize) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.max_buf_size(max_buf_size);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    fn hyper_serverconn_options_pipeline_flush(opts: *mut hyper_serverconn_options, enabled: bool) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.pipeline_flush(enabled);
        hyper_code::HYPERE_OK
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
        let serverconn_options = non_null! { &*serverconn_options ?= ptr::null_mut() };
        let io = non_null! { Box::from_raw(io) ?= ptr::null_mut() };
        let service = non_null! { Box::from_raw(service) ?= ptr::null_mut() };
        let task = hyper_task::boxed(serverconn_options.0.serve_connection(*io, *service));
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

        let (tx, rx) = futures_channel::oneshot::channel();
        let res_channel = Box::into_raw(Box::new(hyper_response_channel(tx)));

        (self.service_fn)(self.userdata.0, req_ptr, res_channel);

        Box::pin(async move {
            let res = rx.await.expect("Channel closed?");
            Ok(res.0)
        })
    }
}
