use hyper::{Body, HeaderMap, Method, Request, Response, Uri};
use libc::size_t;
use std::ffi::c_void;

use crate::{hyper_error, hyper_str};

// ===== impl Request =====

ffi_fn! {
    fn hyper_request_new() -> *mut Request<Body> {
        Box::into_raw(Box::new(Request::new(Body::empty())))
    }
}

ffi_fn! {
    fn hyper_request_free(req: *mut Request<Body>) {
        drop(unsafe { Box::from_raw(req) });
    }
}

ffi_fn! {
    fn hyper_request_set_method(req: *mut Request<Body>, method: *const u8, method_len: size_t) -> hyper_error {
        let bytes = unsafe {
            std::slice::from_raw_parts(method, method_len as usize)
        };
        match Method::from_bytes(bytes) {
            Ok(m) => {
                *unsafe { &mut *req }.method_mut() = m;
                hyper_error::Ok
            },
            Err(_) => {
                hyper_error::Kaboom
            }
        }
    }
}

ffi_fn! {
    fn hyper_request_set_uri(req: *mut Request<Body>, uri: *const u8, uri_len: size_t) -> hyper_error {
        let bytes = unsafe {
            std::slice::from_raw_parts(uri, uri_len as usize)
        };
        match Uri::from_maybe_shared(bytes) {
            Ok(u) => {
                *unsafe { &mut *req }.uri_mut() = u;
                hyper_error::Ok
            },
            Err(_) => {
                hyper_error::Kaboom
            }
        }
    }
}

// ===== impl Response =====

ffi_fn! {
    fn hyper_response_free(resp: *mut Response<Body>) {
        drop(unsafe { Box::from_raw(resp) });
    }
}

ffi_fn! {
    fn hyper_response_status(resp: *const Response<Body>) -> u16 {
        unsafe { &*resp }.status().as_u16()
    }
}

ffi_fn! {
    fn hyper_response_headers(resp: *mut Response<Body>) -> *mut HeaderMap {
        unsafe { &mut *resp }.headers_mut()
    }
}

ffi_fn! {
    fn hyper_response_body(resp: *mut Response<Body>) -> *mut Body {
        unsafe { &mut *resp }.body_mut()
    }
}

// ===== impl Headers =====

#[repr(C)]
#[derive(PartialEq)]
pub enum IterStep {
    Continue = 0,
    #[allow(unused)]
    Break,
}

type IterFn = extern "C" fn(*mut c_void, hyper_str, hyper_str) -> IterStep;

ffi_fn! {
    fn hyper_headers_iter(headers: *const HeaderMap, func: IterFn, userdata: *mut c_void) {
        for (name, value) in unsafe { &*headers }.iter() {
            let raw_name = hyper_str {
                buf: name.as_str().as_bytes().as_ptr(),
                len: name.as_str().as_bytes().len(),
            };
            let raw_val = hyper_str {
                buf: value.as_bytes().as_ptr(),
                len: value.as_bytes().len(),
            };

            if IterStep::Continue != func(userdata, raw_name, raw_val) {
                break;
            }
        }
    }
}
