#[macro_use]
mod macros;

mod body;
mod client;
mod http_types;
mod io;
mod task;

#[repr(C)]
pub struct hyper_str {
    pub buf: *const u8,
    pub len: libc::size_t,
}

#[repr(C)]
pub enum hyper_error {
    Ok = 0,
    Kaboom = 1,
}

impl hyper_str {
    unsafe fn as_slice(&self) -> &[u8] {
        std::slice::from_raw_parts(self.buf, self.len as usize)
    }
}
