use std::ffi::c_void;

/// Many hyper entities can be given userdata to allow user callbacks to correlate work together.
/// Since much of hyper is asynchronous it's often useful to treat these userdata objects as
/// "owned" by the hyper entity (and hence to be cleaned up when that entity is dropped).
///
/// To achieve this a `hyper_userdata_drop` callback is passed by calling code alongside the
/// userdata to register a cleanup function.
///
/// This function may be provided as NULL if the calling code wants to manage memory lifetimes
/// itself, in which case the hyper object will logically consider the userdata "borrowed" until
/// the hyper entity is dropped.
pub type hyper_userdata_drop = Option<extern "C" fn(*mut c_void)>;

/// A handle to a user-provided arbitrary object, along with an optional drop callback for the
/// object.
pub(crate) struct Userdata {
    data: *mut c_void,
    drop: hyper_userdata_drop,
}

impl Userdata {
    pub(crate) fn new(data: *mut c_void, drop: hyper_userdata_drop) -> Self {
        Self {
            data,
            drop,
        }
    }

    pub(crate) fn as_ptr(&self) -> *mut c_void {
        self.data
    }
}

impl Default for Userdata {
    fn default() -> Self {
        Self {
            data: std::ptr::null_mut(),
            drop: None,
        }
    }
}

unsafe impl Sync for Userdata {}
unsafe impl Send for Userdata {}

impl Drop for Userdata {
    fn drop(&mut self) {
        if let Some(drop) = self.drop {
            drop(self.data);
        }
    }
}
