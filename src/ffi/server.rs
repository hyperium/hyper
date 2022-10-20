use std::sync::Arc;
use std::ptr;
use std::ffi::{c_void, c_uint};

use crate::ffi::UserDataPointer;
use crate::ffi::io::hyper_io;
use crate::ffi::error::hyper_code;
use crate::ffi::http_types::{hyper_request, hyper_response};
use crate::ffi::task::{hyper_executor, hyper_task, WeakExec};
use crate::server::conn::Http;

/// Configuration options for server connections.
pub struct hyper_serverconn_options(Http<WeakExec>);

/// A service that can serve a single server connection.
pub struct hyper_service {
    service_fn: hyper_service_callback,
    userdata: UserDataPointer,
}

/// A channel on which to send back a response to complete a transaction for a service.
pub struct hyper_response_channel(futures_channel::oneshot::Sender<Box<hyper_response>>);

/// The main definition of a service.  This callback will be invoked for each transaction on the
/// connection.
///
/// The first argument contains the userdata registered with this service.
///
/// The second argument contains the `hyper_request` that started this transaction.  This request
/// is given to the callback which should free it when it is no longer needed (see
/// [crate::ffi::hyper_request_free]).
///
/// The third argument contains a channel on which a single `hyper_response` must be sent in order
/// to conclude the transaction.  This channel is given to the callback so the sending of the
/// response can be deferred (e.g. by passing it to a different thread, or waiting until other
/// async operations have completed).
pub type hyper_service_callback = extern "C" fn(*mut c_void, *mut hyper_request, *mut hyper_response_channel);

ffi_fn! {
    /// Create a new HTTP serverconn options bound to the provided executor.
    fn hyper_serverconn_options_new(exec: *const hyper_executor) -> *mut hyper_serverconn_options {
        let exec = non_null! { Arc::from_raw(exec) ?= ptr::null_mut() };
        let weak_exec = hyper_executor::downgrade(&exec);
        std::mem::forget(exec); // We've not incremented the strong count when we loaded
                                // `from_raw`
        Box::into_raw(Box::new(hyper_serverconn_options(Http::new().with_executor(weak_exec))))
    }
}

ffi_fn! {
    /// Free a `hyper_serverconn_options*`.
    fn hyper_serverconn_options_free(opts: *mut hyper_serverconn_options) {
        let _ = non_null! { Box::from_raw(opts) ?= () };
    }
}

ffi_fn! {
    /// Configure whether HTTP/1 is required.
    ///
    /// Default is `false`
    fn hyper_serverconn_options_http1_only(opts: *mut hyper_serverconn_options, enabled: bool) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.http1_only(enabled);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    /// Set whether HTTP/1 connections should support half-closures.
    ///
    /// Clients can chose to shutdown their write-side while waiting for the server to respond.
    /// Setting this to true will prevent closing the connection immediately if read detects an EOF
    /// in the middle of a request.
    ///
    /// Default is `false`
    fn hyper_serverconn_options_http1_half_close(opts: *mut hyper_serverconn_options, enabled: bool) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.http1_half_close(enabled);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    /// Enables or disables HTTP/1 keep-alive.
    ///
    /// Default is `true`.
    fn hyper_serverconn_options_http1_keep_alive(opts: *mut hyper_serverconn_options, enabled: bool) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.http1_keep_alive(enabled);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    /// Set whether HTTP/1 connections will write header names as title case at the socket level.
    ///
    /// Note that this setting does not affect HTTP/2.
    ///
    /// Default is `false`.
    fn hyper_serverconn_options_http1_title_case_headers(opts: *mut hyper_serverconn_options, enabled: bool) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.http1_title_case_headers(enabled);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    /// Set whether to support preserving original header cases.
    ///
    /// Currently, this will record the original cases received, and store them in a private
    /// extension on the Request. It will also look for and use such an extension in any provided
    /// Response.
    ///
    /// Since the relevant extension is still private, there is no way to interact with the
    /// original cases. The only effect this can have now is to forward the cases in a proxy-like
    /// fashion.
    ///
    /// Note that this setting does not affect HTTP/2.
    ///
    /// Default is `false`.
    fn hyper_serverconn_options_http1_preserve_header_case(opts: *mut hyper_serverconn_options, enabled: bool) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.http1_preserve_header_case(enabled);
        hyper_code::HYPERE_OK
    }
}

// TODO: http1_header_read_timeout

ffi_fn! {
    /// Set whether HTTP/1 connections should try to use vectored writes, or always flatten into a
    /// single buffer.
    ///
    /// Note that setting this to false may mean more copies of body data, but may also improve
    /// performance when an IO transport doesn’t support vectored writes well, such as most TLS
    /// implementations.
    ///
    /// Setting this to true will force hyper to use queued strategy which may eliminate
    /// unnecessary cloning on some TLS backends.
    ///
    /// Default is to automatically guess which mode to use, this function overrides the huristic.
    fn hyper_serverconn_options_http1_writev(opts: *mut hyper_serverconn_options, enabled: bool) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.http1_writev(enabled);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    /// Set the maximum buffer size for the connection.  Must be no lower `8192`.
    ///
    /// Default is a sensible value.
    fn hyper_serverconn_options_http1_max_buf_size(opts: *mut hyper_serverconn_options, max_buf_size: usize) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.max_buf_size(max_buf_size);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    /// Configure whether HTTP/2 is required.
    ///
    /// Default is `false`.
    fn hyper_serverconn_options_http2_only(opts: *mut hyper_serverconn_options, enabled: bool) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.http2_only(enabled);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    /// Sets the `SETTINGS_INITIAL_WINDOW_SIZE` option for HTTP/2 stream-level flow control.
    ///
    /// Passing `0` instructs hyper to use a sensible default value.
    fn hyper_serverconn_options_http2_initial_stream_window_size(opts: *mut hyper_serverconn_options, window_size: c_uint) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.http2_initial_stream_window_size(if window_size == 0 { None } else { Some(window_size) });
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    /// Sets the max connection-level flow control for HTTP/2.
    ///
    /// Passing `0` instructs hyper to use a sensible default value.
    fn hyper_serverconn_options_http2_initial_connection_window_size(opts: *mut hyper_serverconn_options, window_size: c_uint) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.http2_initial_connection_window_size(if window_size == 0 { None } else { Some(window_size) });
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    /// Sets whether to use an adaptive flow control.
    ///
    /// Enabling this will override the limits set in http2_initial_stream_window_size and
    /// http2_initial_connection_window_size.
    ///
    /// Default is `false`.
    fn hyper_serverconn_options_http2_adaptive_window(opts: *mut hyper_serverconn_options, enabled: bool) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.http2_adaptive_window(enabled);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    /// Sets the maximum frame size to use for HTTP/2.
    ///
    /// Passing `0` instructs hyper to use a sensible default value.
    fn hyper_serverconn_options_http2_max_frame_size(opts: *mut hyper_serverconn_options, frame_size: c_uint) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.http2_max_frame_size(if frame_size == 0 { None } else { Some(frame_size) });
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    /// Sets the `SETTINGS_MAX_CONCURRENT_STREAMS` option for HTTP2 connections.
    ///
    /// Default is no limit (`std::u32::MAX`). Passing `0` will use this default.
    fn hyper_serverconn_options_http2_max_concurrent_streams(opts: *mut hyper_serverconn_options, max_streams: c_uint) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.http2_max_concurrent_streams(if max_streams == 0 { None } else { Some(max_streams) });
        hyper_code::HYPERE_OK
    }
}

// TODO: http2_keep_alive_interval
// TODO: http2_keep_alive_timeout

ffi_fn! {
    /// Set the maximum write buffer size for each HTTP/2 stream.  Must be no larger than
    /// `u32::MAX`.
    ///
    /// Default is a sensible value.
    fn hyper_serverconn_options_http2_max_send_buf_size(opts: *mut hyper_serverconn_options, max_buf_size: usize) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.http2_max_send_buf_size(max_buf_size);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    /// Enables the extended `CONNECT` protocol.
    fn hyper_serverconn_options_http2_enable_connect_protocol(opts: *mut hyper_serverconn_options) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.http2_enable_connect_protocol();
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    /// Sets the max size of received header frames.
    ///
    /// Default is a sensible value.
    fn hyper_serverconn_options_http2_max_header_list_size(opts: *mut hyper_serverconn_options, max: u32) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.http2_max_header_list_size(max);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    /// Aggregates flushes to better support pipelined responses.
    ///
    /// Experimental, may have bugs.
    ///
    /// Default is `false`.
    fn hyper_serverconn_options_pipeline_flush(opts: *mut hyper_serverconn_options, enabled: bool) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.pipeline_flush(enabled);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    /// Create a service from a wrapped callback function.
    fn hyper_service_new(service_fn: hyper_service_callback) -> *mut hyper_service {
        Box::into_raw(Box::new(hyper_service {
            service_fn: service_fn,
            userdata: UserDataPointer(ptr::null_mut()),
        }))
    } ?= ptr::null_mut()
}

ffi_fn! {
    /// Register opaque userdata with the `hyper_service`.
    ///
    /// The service borrows the userdata until the service is driven on a connection and the
    /// associated task completes.
    fn hyper_service_set_userdata(service: *mut hyper_service, userdata: *mut c_void){
        let s = non_null!{ &mut *service ?= () };
        s.userdata = UserDataPointer(userdata);
    }
}

ffi_fn! {
    /// Associate a `hyper_io*` and a `hyper_service*` togther with the options specified in a
    /// `hyper_serverconn_options*`.
    ///
    /// Returns a `hyper_task*` which must be given to an executor to make progress.
    ///
    /// This function consumes the IO and Service objects and thus they should not be accessed
    /// after this function is called.
    fn hyper_serve_connection(serverconn_options: *mut hyper_serverconn_options, io: *mut hyper_io, service: *mut hyper_service) -> *mut hyper_task {
        let serverconn_options = non_null! { &*serverconn_options ?= ptr::null_mut() };
        let io = non_null! { Box::from_raw(io) ?= ptr::null_mut() };
        let service = non_null! { Box::from_raw(service) ?= ptr::null_mut() };
        let task = hyper_task::boxed(serverconn_options.0.serve_connection(*io, *service));
        Box::into_raw(task)
    } ?= ptr::null_mut()
}

ffi_fn! {
    /// Sends a `hyper_response*` back to the client.  This function consumes the response and the
    /// channel.
    ///
    /// See [hyper_service_callback] for details.
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
