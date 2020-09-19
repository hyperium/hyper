macro_rules! ffi_fn {
    (fn $name:ident($($arg:ident: $arg_ty:ty),*) -> $ret:ty $body:block) => {
        #[no_mangle]
        pub extern fn $name($($arg: $arg_ty),*) -> $ret {
            use std::panic::{self, AssertUnwindSafe};

            match panic::catch_unwind(AssertUnwindSafe(move || $body)) {
                Ok(v) => v,
                Err(_) => todo!("FFI UNWIND"),
            }
        }
    };

    (fn $name:ident($($arg:ident: $arg_ty:ty),*) $body:block) => {
        ffi_fn!(fn $name($($arg: $arg_ty),*) -> () $body);
    };
}
