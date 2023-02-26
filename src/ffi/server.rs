use std::ffi::{c_uint, c_void};
use std::ptr;
use std::sync::Arc;

use crate::body::Incoming as IncomingBody;
use crate::ffi::error::hyper_code;
use crate::ffi::http_types::{hyper_request, hyper_response};
use crate::ffi::io::hyper_io;
use crate::ffi::task::{hyper_executor, hyper_task, WeakExec};
use crate::ffi::UserDataPointer;
use crate::server::conn::http1;
use crate::server::conn::http2;

/// Configuration options for HTTP/1 server connections.
pub struct hyper_http1_serverconn_options(http1::Builder);

/// Configuration options for HTTP/2 server connections.
pub struct hyper_http2_serverconn_options(http2::Builder<WeakExec>);

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
pub type hyper_service_callback =
    extern "C" fn(*mut c_void, *mut hyper_request, *mut hyper_response_channel);

// ===== impl http1_serverconn_options =====

ffi_fn! {
    /// Create a new HTTP/1 serverconn options object.
    fn hyper_http1_serverconn_options_new(
        exec: *const hyper_executor
    ) -> *mut hyper_http1_serverconn_options {
        let exec = non_null! { Arc::from_raw(exec) ?= ptr::null_mut() };
        let mut builder = http1::Builder::new();
        builder.timer(Arc::clone(exec.timer_heap()));
        std::mem::forget(exec); // We never incremented the strong count in this function so can't
                                // drop our Arc.
        Box::into_raw(Box::new(hyper_http1_serverconn_options(
            builder
        )))
    }
}

ffi_fn! {
    /// Free a `hyper_http1_serverconn_options*`.
    fn hyper_http1_serverconn_options_free(opts: *mut hyper_http1_serverconn_options) {
        let _ = non_null! { Box::from_raw(opts) ?= () };
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
    fn hyper_http1_serverconn_options_half_close(
        opts: *mut hyper_http1_serverconn_options,
        enabled: bool,
    ) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.half_close(enabled);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    /// Enables or disables HTTP/1 keep-alive.
    ///
    /// Default is `true`.
    fn hyper_http1_serverconn_options_keep_alive(
        opts: *mut hyper_http1_serverconn_options,
        enabled: bool,
    ) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.keep_alive(enabled);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    /// Set whether HTTP/1 connections will write header names as title case at the socket level.
    ///
    /// Default is `false`.
    fn hyper_http1_serverconn_options_title_case_headers(
        opts: *mut hyper_http1_serverconn_options,
        enabled: bool,
    ) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.title_case_headers(enabled);
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
    /// Default is `false`.
    fn hyper_http1_serverconn_options_preserve_header_case(
        opts: *mut hyper_http1_serverconn_options,
        enabled: bool,
    ) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.preserve_header_case(enabled);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    /// Set a timeout for reading client request headers. If a client does not
    /// transmit the entire header within this time, the connection is closed.
    ///
    /// Default is to have no timeout.
    fn hyper_http1_serverconn_options_header_read_timeout(
        opts: *mut hyper_http1_serverconn_options,
        millis: u64,
    ) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.header_read_timeout(std::time::Duration::from_millis(millis));
        hyper_code::HYPERE_OK
    }
}

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
    fn hyper_http1_serverconn_options_writev(
        opts: *mut hyper_http1_serverconn_options,
        enabled: bool,
    ) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.writev(enabled);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    /// Set the maximum buffer size for the HTTP/1 connection.  Must be no lower `8192`.
    ///
    /// Default is a sensible value.
    fn hyper_http1_serverconn_options_max_buf_size(
        opts: *mut hyper_http1_serverconn_options,
        max_buf_size: usize,
    ) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.max_buf_size(max_buf_size);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    /// Aggregates flushes to better support pipelined responses.
    ///
    /// Experimental, may have bugs.
    ///
    /// Default is `false`.
    fn hyper_http1_serverconn_options_pipeline_flush(
        opts: *mut hyper_http1_serverconn_options,
        enabled: bool,
    ) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.pipeline_flush(enabled);
        hyper_code::HYPERE_OK
    }
}

// ===== impl hyper_http2_serverconn_options =====

ffi_fn! {
    /// Create a new HTTP/2 serverconn options object bound to the provided executor.
    fn hyper_http2_serverconn_options_new(
        exec: *const hyper_executor,
    ) -> *mut hyper_http2_serverconn_options {
        let exec = non_null! { Arc::from_raw(exec) ?= ptr::null_mut() };
        let weak = hyper_executor::downgrade(&exec);
        let mut builder = http2::Builder::new(weak.clone());
        builder.timer(Arc::clone(exec.timer_heap()));
        std::mem::forget(exec); // We never incremented the strong count in this function so can't
                                // drop our Arc.
        Box::into_raw(Box::new(hyper_http2_serverconn_options(
            builder
        )))
    }
}

ffi_fn! {
    /// Free a `hyper_http2_serverconn_options*`.
    fn hyper_http2_serverconn_options_free(opts: *mut hyper_http2_serverconn_options) {
        let _ = non_null! { Box::from_raw(opts) ?= () };
    }
}

ffi_fn! {
    /// Sets the `SETTINGS_INITIAL_WINDOW_SIZE` option for HTTP/2 stream-level flow control.
    ///
    /// Passing `0` instructs hyper to use a sensible default value.
    fn hyper_http2_serverconn_options_initial_stream_window_size(
        opts: *mut hyper_http2_serverconn_options,
        window_size: c_uint,
    ) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0
            .initial_stream_window_size(if window_size == 0 {
                None
            } else {
                Some(window_size)
            });
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    /// Sets the max connection-level flow control for HTTP/2.
    ///
    /// Passing `0` instructs hyper to use a sensible default value.
    fn hyper_http2_serverconn_options_initial_connection_window_size(
        opts: *mut hyper_http2_serverconn_options,
        window_size: c_uint,
    ) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0
            .initial_connection_window_size(if window_size == 0 {
                None
            } else {
                Some(window_size)
            });
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
    fn hyper_http2_serverconn_options_adaptive_window(
        opts: *mut hyper_http2_serverconn_options,
        enabled: bool,
    ) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.adaptive_window(enabled);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    /// Sets the maximum frame size to use for HTTP/2.
    ///
    /// Passing `0` instructs hyper to use a sensible default value.
    fn hyper_http2_serverconn_options_max_frame_size(
        opts: *mut hyper_http2_serverconn_options,
        frame_size: c_uint,
    ) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.max_frame_size(if frame_size == 0 { None } else { Some(frame_size) });
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    /// Sets the `SETTINGS_MAX_CONCURRENT_STREAMS` option for HTTP2 connections.
    ///
    /// Default is no limit (`std::u32::MAX`). Passing `0` will use this default.
    fn hyper_http2_serverconn_options_max_concurrent_streams(
        opts: *mut hyper_http2_serverconn_options,
        max_streams: c_uint,
    ) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.max_concurrent_streams(if max_streams == 0 {
            None
        } else {
            Some(max_streams)
        });
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    /// Sets an interval for HTTP/2 Ping frames should be sent to keep a connection alive.
    ///
    /// Default is to not use keepalive pings.  Passing `0` will use this default.
    fn hyper_http2_serverconn_options_keep_alive_interval(
        opts: *mut hyper_http2_serverconn_options,
        interval_seconds: u64,
    ) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.keep_alive_interval(if interval_seconds == 0 {
            None
        } else {
            Some(std::time::Duration::from_secs(interval_seconds))
        });
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    /// Sets a timeout for receiving an acknowledgement of the keep-alive ping.
    ///
    /// If the ping is not acknowledged within the timeout, the connection will be closed. Does
    /// nothing if `hyper_http2_serverconn_options_keep_alive_interval` is disabled.
    ///
    /// Default is 20 seconds.
    fn hyper_http2_serverconn_options_keep_alive_timeout(
        opts: *mut hyper_http2_serverconn_options,
        timeout_seconds: u64,
    ) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.keep_alive_timeout(std::time::Duration::from_secs(timeout_seconds));
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    /// Set the maximum write buffer size for each HTTP/2 stream.  Must be no larger than
    /// `u32::MAX`.
    ///
    /// Default is a sensible value.
    fn hyper_http2_serverconn_options_max_send_buf_size(
        opts: *mut hyper_http2_serverconn_options,
        max_buf_size: usize,
    ) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.max_send_buf_size(max_buf_size);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    /// Enables the extended `CONNECT` protocol.
    fn hyper_http2_serverconn_options_enable_connect_protocol(
        opts: *mut hyper_http2_serverconn_options,
    ) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.enable_connect_protocol();
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    /// Sets the max size of received header frames.
    ///
    /// Default is a sensible value.
    fn hyper_http2_serverconn_options_max_header_list_size(
        opts: *mut hyper_http2_serverconn_options,
        max: u32,
    ) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.0.max_header_list_size(max);
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
    /// Serve the provided `hyper_service *` as an HTTP/1 endpoint over the provided `hyper_io *`
    /// and configured as per the `hyper_http1_serverconn_options *`.
    ///
    /// Returns a `hyper_task*` which must be given to an executor to make progress.
    ///
    /// This function consumes the IO and Service objects and thus they should not be accessed
    /// after this function is called.
    fn hyper_serve_http1_connection(
        serverconn_options: *mut hyper_http1_serverconn_options,
        io: *mut hyper_io,
        service: *mut hyper_service,
    ) -> *mut hyper_task {
        let serverconn_options = non_null! { &*serverconn_options ?= ptr::null_mut() };
        let io = non_null! { Box::from_raw(io) ?= ptr::null_mut() };
        let service = non_null! { Box::from_raw(service) ?= ptr::null_mut() };
        let task = hyper_task::boxed(serverconn_options.0.serve_connection(*io, *service));
        Box::into_raw(task)
    } ?= ptr::null_mut()
}

ffi_fn! {
    /// Serve the provided `hyper_service *` as an HTTP/2 endpoint over the provided `hyper_io *`
    /// and configured as per the `hyper_http2_serverconn_options *`.
    ///
    /// Returns a `hyper_task*` which must be given to an executor to make progress.
    ///
    /// This function consumes the IO and Service objects and thus they should not be accessed
    /// after this function is called.
    fn hyper_serve_http2_connection(
        serverconn_options: *mut hyper_http2_serverconn_options,
        io: *mut hyper_io,
        service: *mut hyper_service,
    ) -> *mut hyper_task {
        let serverconn_options = non_null! { &*serverconn_options ?= ptr::null_mut() };
        let io = non_null! { Box::from_raw(io) ?= ptr::null_mut() };
        let service = non_null! { Box::from_raw(service) ?= ptr::null_mut() };
        let task = hyper_task::boxed(serverconn_options.0.serve_connection(*io, *service));
        Box::into_raw(task)
    } ?= ptr::null_mut()
}

ffi_fn! {
    /// Serve the provided `hyper_service *` as either an HTTP/1 or HTTP/2 (depending on what the
    /// client requests) endpoint over the provided `hyper_io *` and configured as per the
    /// appropriate `hyper_httpX_serverconn_options *`.
    ///
    /// Returns a `hyper_task*` which must be given to an executor to make progress.
    ///
    /// This function consumes the IO and Service objects and thus they should not be accessed
    /// after this function is called.
    fn hyper_serve_httpX_connection(
        http1_serverconn_options: *mut hyper_http1_serverconn_options,
        http2_serverconn_options: *mut hyper_http2_serverconn_options,
        io: *mut hyper_io,
        service: *mut hyper_service,
    ) -> *mut hyper_task {
        let http1_serverconn_options = non_null! { &*http1_serverconn_options ?= ptr::null_mut() };
        let http2_serverconn_options = non_null! { &*http2_serverconn_options ?= ptr::null_mut() };
        let io = non_null! { Box::from_raw(io) ?= ptr::null_mut() };
        let service = non_null! { Box::from_raw(service) ?= ptr::null_mut() };
        let task = hyper_task::boxed(
            AutoConnection::H1(
                Some((
                    http1_serverconn_options.0.serve_connection(*io, *service),
                    http2_serverconn_options.0.clone()
                ))
            )
        );
        Box::into_raw(task)
    } ?= ptr::null_mut()
}

ffi_fn! {
    /// Sends a `hyper_response*` back to the client.  This function consumes the response and the
    /// channel.
    ///
    /// See [hyper_service_callback] for details.
    fn hyper_response_channel_send(
        channel: *mut hyper_response_channel,
        response: *mut hyper_response,
    ) {
        let channel = non_null! { Box::from_raw(channel) ?= () };
        let response = non_null! { Box::from_raw(response) ?= () };
        let _ = channel.0.send(response);
    }
}

impl crate::service::Service<crate::Request<IncomingBody>> for hyper_service {
    type Response = crate::Response<IncomingBody>;
    type Error = crate::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn call(&mut self, req: crate::Request<IncomingBody>) -> Self::Future {
        let req_ptr = Box::into_raw(Box::new(hyper_request::from(req)));

        let (tx, rx) = futures_channel::oneshot::channel();
        let rsp_channel = Box::into_raw(Box::new(hyper_response_channel(tx)));

        (self.service_fn)(self.userdata.0, req_ptr, rsp_channel);

        Box::pin(async move {
            let rsp = rx.await.expect("Channel closed?");
            Ok(rsp.finalize())
        })
    }
}

enum AutoConnection<IO, Serv, Exec>
where
    Serv: crate::service::HttpService<IncomingBody>,
{
    // The internals are in an `Option` so they can be extracted during H1->H2 fallback. Otherwise
    // this must always be `Some(h1, h2)` (and code is allowed to panic if that's not true).
    H1(Option<(http1::Connection<IO, Serv>, http2::Builder<Exec>)>),
    H2(http2::Connection<crate::common::io::Rewind<IO>, Serv, Exec>),
}

impl<IO, Serv, Exec> std::future::Future for AutoConnection<IO, Serv, Exec>
where
    IO: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + 'static,
    Serv: crate::service::HttpService<IncomingBody, ResBody = IncomingBody>,
    Exec: crate::rt::Executor<crate::proto::h2::server::H2Stream<Serv::Future, IncomingBody>> + Unpin + Clone,
    http1::Connection<IO, Serv>: std::future::Future<Output = Result<(), crate::Error>> + Unpin,
    http2::Connection<crate::common::io::Rewind<IO>, Serv, Exec>:
        std::future::Future<Output = Result<(), crate::Error>> + Unpin,
{
    type Output = crate::Result<()>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let zelf = std::pin::Pin::into_inner(self);
        let (h1, h2) = match zelf {
            AutoConnection::H1(inner) => {
                match ready!(std::pin::Pin::new(&mut inner.as_mut().unwrap().0).poll(cx)) {
                    Ok(()) => return std::task::Poll::Ready(Ok(())),
                    Err(e) => {
                        let kind = e.kind();
                        if matches!(
                            kind,
                            crate::error::Kind::Parse(crate::error::Parse::VersionH2)
                        ) {
                            // Fallback - switching variant has to happen outside the match block since
                            // `self` is borrowed.
                            //
                            // This breaks the invariant of the H1 variant, so we _must_ fix up `zelf`
                            // before returning from this function.
                            inner.take().unwrap()
                        } else {
                            // Some other error, pass upwards
                            return std::task::Poll::Ready(Err(e));
                        }
                    }
                }
            }
            AutoConnection::H2(h2) => match ready!(std::pin::Pin::new(h2).poll(cx)) {
                Ok(()) => return std::task::Poll::Ready(Ok(())),
                Err(e) => return std::task::Poll::Ready(Err(e)),
            },
        };

        // We've not returned already (for pending, success or "other" errors) so we must be
        // switching to H2 - rewind the IO, build an H2 connection, update `zelf` to the H2 variant
        // then re-schedule this future for mainline processing.
        let http1::Parts {
            io,
            read_buf,
            service,
            ..
        } = h1.into_parts();
        let rewind = crate::common::io::Rewind::new_buffered(io, read_buf);
        let h2 = h2.serve_connection(rewind, service);
        *zelf = AutoConnection::H2(h2);
        std::pin::Pin::new(zelf).poll(cx)
    }
}
