#[repr(C)]
pub struct hyper_waker {
    _p: [u8; 16],
}

pub struct Exec;
pub struct Task;

/// typedef enum hyper_return_task_type
#[repr(C)]
pub enum TaskType {
    Bg,
}

// ===== impl Exec =====

ffi_fn! {
    fn hyper_executor_new() -> *mut Exec {
        Box::into_raw(Box::new(Exec))
    }
}

ffi_fn! {
    fn hyper_executor_free(exec: *mut Exec) {
        drop(unsafe { Box::from_raw(exec) });
    }
}

ffi_fn! {
    fn hyper_executor_push(_exec: *mut Exec, _task: *mut Task) {
    }
}

ffi_fn! {
    fn hyper_executor_poll(_exec: *mut Exec) {
    }
}

ffi_fn! {
    fn hyper_executor_pop(_exec: *mut Exec) -> *mut Task {
        std::ptr::null_mut()
    }
}

// ===== impl Task =====

impl Task {
    pub(crate) fn boxed<T>(_: T) -> Box<Task> {
        todo!("boxed")
    }
}
ffi_fn! {
    fn hyper_task_free(task: *mut Task) {
        drop(unsafe { Box::from_raw(task) });
    }
}

ffi_fn! {
    fn hyper_task_value(_task: *mut Task) -> *mut () {
        std::ptr::null_mut()
    }
}

ffi_fn! {
    fn hyper_task_type(_task: *mut Task) -> TaskType {
        TaskType::Bg
    }
}
