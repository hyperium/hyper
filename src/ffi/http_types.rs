use libc::{c_int, size_t};
use std::ffi::c_void;

use super::body::hyper_body;
use super::error::hyper_code;
use super::task::{hyper_task_return_type, AsTaskType};
use super::HYPER_ITER_CONTINUE;
use crate::header::{HeaderName, HeaderValue};
use crate::{Body, HeaderMap, Method, Request, Response, Uri};

// ===== impl Request =====

pub struct hyper_request(pub(super) Request<Body>);

pub struct hyper_response(pub(super) Response<Body>);

pub struct hyper_headers(pub(super) HeaderMap);

ffi_fn! {
    /// Construct a new HTTP request.
    fn hyper_request_new() -> *mut hyper_request {
        Box::into_raw(Box::new(hyper_request(Request::new(Body::empty()))))
    }
}

ffi_fn! {
    /// Free an HTTP request if not going to send it on a client.
    fn hyper_request_free(req: *mut hyper_request) {
        drop(unsafe { Box::from_raw(req) });
    }
}

ffi_fn! {
    /// Set the HTTP Method of the request.
    fn hyper_request_set_method(req: *mut hyper_request, method: *const u8, method_len: size_t) -> hyper_code {
        let bytes = unsafe {
            std::slice::from_raw_parts(method, method_len as usize)
        };
        match Method::from_bytes(bytes) {
            Ok(m) => {
                *unsafe { &mut *req }.0.method_mut() = m;
                hyper_code::HYPERE_OK
            },
            Err(_) => {
                hyper_code::HYPERE_INVALID_ARG
            }
        }
    }
}

ffi_fn! {
    /// Set the URI of the request.
    fn hyper_request_set_uri(req: *mut hyper_request, uri: *const u8, uri_len: size_t) -> hyper_code {
        let bytes = unsafe {
            std::slice::from_raw_parts(uri, uri_len as usize)
        };
        match Uri::from_maybe_shared(bytes) {
            Ok(u) => {
                *unsafe { &mut *req }.0.uri_mut() = u;
                hyper_code::HYPERE_OK
            },
            Err(_) => {
                hyper_code::HYPERE_INVALID_ARG
            }
        }
    }
}

ffi_fn! {
    /// Set the preferred HTTP version of the request.
    ///
    /// The version value should be one of the `HYPER_HTTP_VERSION_` constants.
    ///
    /// Note that this won't change the major HTTP version of the connection,
    /// since that is determined at the handshake step.
    fn hyper_request_set_version(req: *mut hyper_request, version: c_int) -> hyper_code {
        use http::Version;

        *unsafe { &mut *req }.0.version_mut() = match version {
            super::HYPER_HTTP_VERSION_NONE => Version::HTTP_11,
            super::HYPER_HTTP_VERSION_1_0 => Version::HTTP_10,
            super::HYPER_HTTP_VERSION_1_1 => Version::HTTP_11,
            super::HYPER_HTTP_VERSION_2 => Version::HTTP_2,
            _ => {
                // We don't know this version
                return hyper_code::HYPERE_INVALID_ARG;
            }
        };
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    /// Gets a reference to the HTTP headers of this request
    ///
    /// This is not an owned reference, so it should not be accessed after the
    /// `hyper_request` has been consumed.
    fn hyper_request_headers(req: *mut hyper_request) -> *mut hyper_headers {
        hyper_headers::wrap(unsafe { &mut *req }.0.headers_mut())
    }
}

ffi_fn! {
    /// Set the body of the request.
    ///
    /// The default is an empty body.
    ///
    /// This takes ownership of the `hyper_body *`, you must not use it or
    /// free it after setting it on the request.
    fn hyper_request_set_body(req: *mut hyper_request, body: *mut hyper_body) -> hyper_code {
        let body = unsafe { Box::from_raw(body) };
        *unsafe { &mut *req }.0.body_mut() = body.0;
        hyper_code::HYPERE_OK
    }
}

// ===== impl Response =====

ffi_fn! {
    /// Free an HTTP response after using it.
    fn hyper_response_free(resp: *mut hyper_response) {
        drop(unsafe { Box::from_raw(resp) });
    }
}

ffi_fn! {
    /// Get the HTTP-Status code of this response.
    ///
    /// It will always be within the range of 100-599.
    fn hyper_response_status(resp: *const hyper_response) -> u16 {
        unsafe { &*resp }.0.status().as_u16()
    }
}

ffi_fn! {
    /// Get the HTTP version used by this response.
    ///
    /// The returned value could be:
    ///
    /// - `HYPER_HTTP_VERSION_1_0`
    /// - `HYPER_HTTP_VERSION_1_1`
    /// - `HYPER_HTTP_VERSION_2`
    /// - `HYPER_HTTP_VERSION_NONE` if newer (or older).
    fn hyper_response_version(resp: *const hyper_response) -> c_int {
        use http::Version;

        match unsafe { &*resp }.0.version() {
            Version::HTTP_10 => super::HYPER_HTTP_VERSION_1_0,
            Version::HTTP_11 => super::HYPER_HTTP_VERSION_1_1,
            Version::HTTP_2 => super::HYPER_HTTP_VERSION_2,
            _ => super::HYPER_HTTP_VERSION_NONE,
        }
    }
}

ffi_fn! {
    /// Gets a reference to the HTTP headers of this response.
    ///
    /// This is not an owned reference, so it should not be accessed after the
    /// `hyper_response` has been freed.
    fn hyper_response_headers(resp: *mut hyper_response) -> *mut hyper_headers {
        hyper_headers::wrap(unsafe { &mut *resp }.0.headers_mut())
    }
}

ffi_fn! {
    /// Take ownership of the body of this response.
    ///
    /// It is safe to free the response even after taking ownership of its body.
    fn hyper_response_body(resp: *mut hyper_response) -> *mut hyper_body {
        let body = std::mem::take(unsafe { &mut *resp }.0.body_mut());
        Box::into_raw(Box::new(hyper_body(body)))
    }
}

unsafe impl AsTaskType for hyper_response {
    fn as_task_type(&self) -> hyper_task_return_type {
        hyper_task_return_type::HYPER_TASK_RESPONSE
    }
}

// ===== impl Headers =====

type hyper_headers_foreach_callback =
    extern "C" fn(*mut c_void, *const u8, size_t, *const u8, size_t) -> c_int;

impl hyper_headers {
    pub(crate) fn wrap(cx: &mut HeaderMap) -> &mut hyper_headers {
        // A struct with only one field has the same layout as that field.
        unsafe { std::mem::transmute::<&mut HeaderMap, &mut hyper_headers>(cx) }
    }
}

ffi_fn! {
    /// Iterates the headers passing each name and value pair to the callback.
    ///
    /// The `userdata` pointer is also passed to the callback.
    ///
    /// The callback should return `HYPER_ITER_CONTINUE` to keep iterating, or
    /// `HYPER_ITER_BREAK` to stop.
    fn hyper_headers_foreach(headers: *const hyper_headers, func: hyper_headers_foreach_callback, userdata: *mut c_void) {
        for (name, value) in unsafe { &*headers }.0.iter() {
            let name_ptr = name.as_str().as_bytes().as_ptr();
            let name_len = name.as_str().as_bytes().len();
            let val_ptr = value.as_bytes().as_ptr();
            let val_len = value.as_bytes().len();

            if HYPER_ITER_CONTINUE != func(userdata, name_ptr, name_len, val_ptr, val_len) {
                break;
            }
        }
    }
}

ffi_fn! {
    /// Sets the header with the provided name to the provided value.
    ///
    /// This overwrites any previous value set for the header.
    fn hyper_headers_set(headers: *mut hyper_headers, name: *const u8, name_len: size_t, value: *const u8, value_len: size_t) -> hyper_code {
        let headers = unsafe { &mut *headers };
        match unsafe { raw_name_value(name, name_len, value, value_len) } {
            Ok((name, value)) => {
                headers.0.insert(name, value);
                hyper_code::HYPERE_OK
            }
            Err(code) => code,
        }
    }
}

ffi_fn! {
    /// Adds the provided value to the list of the provided name.
    ///
    /// If there were already existing values for the name, this will append the
    /// new value to the internal list.
    fn hyper_headers_add(headers: *mut hyper_headers, name: *const u8, name_len: size_t, value: *const u8, value_len: size_t) -> hyper_code {
        let headers = unsafe { &mut *headers };

        match unsafe { raw_name_value(name, name_len, value, value_len) } {
            Ok((name, value)) => {
                headers.0.append(name, value);
                hyper_code::HYPERE_OK
            }
            Err(code) => code,
        }
    }
}

unsafe fn raw_name_value(
    name: *const u8,
    name_len: size_t,
    value: *const u8,
    value_len: size_t,
) -> Result<(HeaderName, HeaderValue), hyper_code> {
    let name = std::slice::from_raw_parts(name, name_len);
    let name = match HeaderName::from_bytes(name) {
        Ok(name) => name,
        Err(_) => return Err(hyper_code::HYPERE_INVALID_ARG),
    };
    let value = std::slice::from_raw_parts(value, value_len);
    let value = match HeaderValue::from_bytes(value) {
        Ok(val) => val,
        Err(_) => return Err(hyper_code::HYPERE_INVALID_ARG),
    };

    Ok((name, value))
}
