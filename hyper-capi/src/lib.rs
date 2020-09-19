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
