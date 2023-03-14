use std::ffi::c_void;

/// Many hyper entites can be given userdata to allow user callbacks to corellate work together.
/// Since much of hyper is asychronous it's often useful to treat these userdata objects as "owned"
/// by the hyper entity (and hence to be cleaned up when that entity is dropped).
///
/// To acheive this a `hyepr_userdata_drop` callback is passed by calling code alongside the
/// userdata to register a cleanup function.
///
/// This function may be provided as NULL if the calling code wants to manage memory lifetimes
/// itself, in which case the hyper object will logically consider the userdata "borrowed" until
/// the hyper entity is dropped.
pub type hyper_userdata_drop = extern "C" fn(*mut c_void);

/// A handle to a user-provided arbitrary object, along with an optional drop callback for the
/// object.
pub(crate) struct Userdata {
    data: *mut c_void,
    drop: Option<hyper_userdata_drop>,
}

impl Userdata {
    pub(crate) fn new(data: *mut c_void, drop: hyper_userdata_drop) -> Self {
        Self {
            data,
            drop: if (drop as *const c_void).is_null() {
                None
            } else {
                Some(drop)
            }
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
