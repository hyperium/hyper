use std::ffi::c_void;
use std::future::Future;
use std::mem::ManuallyDrop;
use std::pin::Pin;
use std::ptr;
use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex, Weak};
use std::task::{Context, Poll};

use futures_util::stream::{FuturesUnordered, Stream};

use crate::{AssertSendSafe, hyper_code};

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;
type BoxAny = Box<dyn AsTaskType + Send + Sync>;

pub struct Exec {
    /// The executor of all task futures.
    ///
    /// There should never be contention on the mutex, as it is only locked
    /// to drive the futures. However, we cannot gaurantee proper usage from
    /// `hyper_executor_poll()`, which in C could potentially be called inside
    /// one of the stored futures. The mutex isn't re-entrant, so doing so
    /// would result in a deadlock, but that's better than data corruption.
    driver: Mutex<FuturesUnordered<TaskFuture>>,

    /// The queue of futures that need to be pushed into the `driver`.
    ///
    /// This is has a separate mutex since `spawn` could be called from inside
    /// a future, which would mean the driver's mutex is already locked.
    spawn_queue: Mutex<Vec<TaskFuture>>,

    /// This is used to track when a future calls `wake` while we are within
    /// `Exec::poll_next`.
    is_woken: AtomicBool,
}

#[derive(Clone)]
pub(crate) struct WeakExec(Weak<Exec>);

pub struct Task {
    future: BoxFuture<BoxAny>,
    output: Option<BoxAny>,
    userdata: AssertSendSafe<*mut c_void>,
}

struct TaskFuture {
    task: Option<Box<Task>>,
}

pub struct Waker {
    waker: std::task::Waker,
}

/// typedef enum hyper_return_task_type
#[repr(C)]
pub enum TaskType {
    Empty,
    Error,
    ClientConn,
    Response,
}

pub(crate) unsafe trait AsTaskType {
    fn as_task_type(&self) -> TaskType;
}

pub(crate) trait IntoDynTaskType {
    fn into_dyn_task_type(self) -> BoxAny;
}

// ===== impl Exec =====

impl Exec {
    fn new() -> Arc<Exec> {
        Arc::new(Exec {
            driver: Mutex::new(FuturesUnordered::new()),
            spawn_queue: Mutex::new(Vec::new()),
            is_woken: AtomicBool::new(false),
        })
    }

    pub(crate) fn downgrade(exec: &Arc<Exec>) -> WeakExec {
        WeakExec(Arc::downgrade(exec))
    }

    fn spawn(&self, task: Box<Task>) {
        self.spawn_queue
            .lock()
            .unwrap()
            .push(TaskFuture { task: Some(task) });
    }

    fn poll_next(arc_self: &Arc<Exec>) -> Option<Box<Task>> {
        let me = &*arc_self;
        // Drain the queue first.
        me.drain_queue();

        let waker = futures_util::task::waker_ref(arc_self);
        let mut cx = Context::from_waker(&waker);

        loop {
            match Pin::new(&mut *me.driver.lock().unwrap()).poll_next(&mut cx) {
                Poll::Ready(val) => return val,
                Poll::Pending => {
                    // Check if any of the pending tasks tried to spawn
                    // some new tasks. If so, drain into the driver and loop.
                    if me.drain_queue() {
                        continue;
                    }

                    // If the driver called `wake` while we were polling,
                    // we should poll again immediately!
                    if me.is_woken.swap(false, Ordering::SeqCst) {
                        continue;
                    }

                    return None;
                }
            }
        }
    }

    fn drain_queue(&self) -> bool {
        let mut queue = self.spawn_queue.lock().unwrap();
        if queue.is_empty() {
            return false;
        }

        let driver = self.driver.lock().unwrap();

        for task in queue.drain(..) {
            driver.push(task);
        }

        true
    }
}

impl futures_util::task::ArcWake for Exec {
    fn wake_by_ref(me: &Arc<Exec>) {
        me.is_woken.store(true, Ordering::SeqCst);
    }
}

// ===== impl WeakExec =====

impl WeakExec {
    pub(crate) fn new() -> Self {
        WeakExec(Weak::new())
    }
}

impl hyper::rt::Executor<BoxFuture<()>> for WeakExec {
    fn execute(&self, fut: BoxFuture<()>) {
        if let Some(exec) = self.0.upgrade() {
            exec.spawn(Task::boxed(fut));
        }
    }
}

ffi_fn! {
    fn hyper_executor_new() -> *const Exec {
        Arc::into_raw(Exec::new())
    }
}

ffi_fn! {
    fn hyper_executor_free(exec: *const Exec) {
        drop(unsafe { Arc::from_raw(exec) });
    }
}

ffi_fn! {
    fn hyper_executor_push(exec: *const Exec, task: *mut Task) -> hyper_code {
        if exec.is_null() || task.is_null() {
            return hyper_code::Kaboom;
        }
        let exec = unsafe { &*exec };
        let task = unsafe { Box::from_raw(task) };
        exec.spawn(task);
        hyper_code::Ok
    }
}

ffi_fn! {
    fn hyper_executor_poll(exec: *const Exec) -> *mut Task {
        // We only want an `&Arc` in here, so wrap in a `ManuallyDrop` so we
        // don't accidentally trigger a ref_dec of the Arc.
        let exec = ManuallyDrop::new(unsafe { Arc::from_raw(exec) });
        match Exec::poll_next(&*exec) {
            Some(task) => Box::into_raw(task),
            None => ptr::null_mut(),
        }
    }
}

// ===== impl Task =====

impl Task {
    pub(crate) fn boxed<F>(fut: F) -> Box<Task>
    where
        F: Future + Send + 'static,
        F::Output: IntoDynTaskType + Send + Sync + 'static,
    {
        Box::new(Task {
            future: Box::pin(async move { fut.await.into_dyn_task_type() }),
            output: None,
            userdata: AssertSendSafe(ptr::null_mut()),
        })
    }

    fn output_type(&self) -> TaskType {
        match self.output {
            None => TaskType::Empty,
            Some(ref val) => val.as_task_type(),
        }
    }
}

impl Future for TaskFuture {
    type Output = Box<Task>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.task.as_mut().unwrap().future).poll(cx) {
            Poll::Ready(val) => {
                let mut task = self.task.take().unwrap();
                task.output = Some(val);
                Poll::Ready(task)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

ffi_fn! {
    fn hyper_task_free(task: *mut Task) {
        drop(unsafe { Box::from_raw(task) });
    }
}

ffi_fn! {
    fn hyper_task_value(task: *mut Task) -> *mut c_void {
        if task.is_null() {
            return ptr::null_mut();
        }

        let task = unsafe { &mut *task };

        if let Some(val) = task.output.take() {
            Box::into_raw(val) as *mut c_void
        } else {
            ptr::null_mut()
        }
    }
}

ffi_fn! {
    fn hyper_task_type(task: *mut Task) -> TaskType {
        if task.is_null() {
            // instead of blowing up spectacularly, just say this null task
            // doesn't have a value to retrieve.
            return TaskType::Empty;
        }

        unsafe { &*task }.output_type()
    }
}

ffi_fn! {
    fn hyper_task_set_data(task: *mut Task, userdata: *mut c_void) {
        if task.is_null() {
            return;
        }

        unsafe { (*task).userdata = AssertSendSafe(userdata) };
    }
}

ffi_fn! {
    fn hyper_task_userdata(task: *mut Task) -> *mut c_void {
        if task.is_null() {
            return ptr::null_mut();
        }

        unsafe { &*task }.userdata.0
    }
}

// ===== impl AsTaskType =====

unsafe impl AsTaskType for () {
    fn as_task_type(&self) -> TaskType {
        TaskType::Empty
    }
}

unsafe impl AsTaskType for hyper::Error {
    fn as_task_type(&self) -> TaskType {
        TaskType::Error
    }
}

impl<T> IntoDynTaskType for T
where
    T: AsTaskType + Send + Sync + 'static,
{
    fn into_dyn_task_type(self) -> BoxAny {
        Box::new(self)
    }
}

impl<T> IntoDynTaskType for hyper::Result<T>
where
    T: AsTaskType + Send + Sync + 'static,
{
    fn into_dyn_task_type(self) -> BoxAny {
        match self {
            Ok(val) => Box::new(val),
            Err(err) => Box::new(err), //TaskType::Error,
        }
    }
}

// ===== impl Waker =====


ffi_fn! {
    fn hyper_context_waker(cx: *mut Context<'_>) -> *mut Waker {
        let waker = unsafe { &mut *cx }.waker().clone();
        Box::into_raw(Box::new(Waker { waker }))
    }
}

ffi_fn! {
    fn hyper_waker_free(waker: *mut Waker) {
        drop(unsafe { Box::from_raw(waker) });
    }
}

ffi_fn! {
    fn hyper_waker_wake(waker: *mut Waker) {
        let waker = unsafe { Box::from_raw(waker) };
        waker.waker.wake();
    }
}
