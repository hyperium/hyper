#[macro_use]
mod macros;

mod body;
mod client;
mod http_types;
mod io;
mod task;

#[repr(C)]
pub enum hyper_error {
    Ok = 0,
    Kaboom = 1,
}
