use std::ffi::c_void;
use std::future::Future;
use std::pin::Pin;
use std::ptr;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, Weak,
};
use std::task::{Context, Poll};

use futures_util::stream::{FuturesUnordered, Stream};
use libc::c_int;

use super::error::hyper_code;
use super::userdata::{Userdata, hyper_userdata_drop};

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;
type BoxAny = Box<dyn AsTaskType + Send + Sync>;

/// Return in a poll function to indicate it was ready.
pub const HYPER_POLL_READY: c_int = 0;
/// Return in a poll function to indicate it is still pending.
///
/// The passed in `hyper_waker` should be registered to wake up the task at
/// some later point.
pub const HYPER_POLL_PENDING: c_int = 1;
/// Return in a poll function indicate an error.
pub const HYPER_POLL_ERROR: c_int = 3;

/// A task executor for `hyper_task`s.
pub struct hyper_executor {
    /// The executor of all task futures.
    ///
    /// There should never be contention on the mutex, as it is only locked
    /// to drive the futures. However, we cannot guarantee proper usage from
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
    /// `hyper_executor::poll_next`.
    is_woken: Arc<ExecWaker>,

    /// The heap of programmed timers, these will be progressed at the start of
    /// `hyper_executor_poll`
    timers: Arc<Mutex<crate::ffi::time::TimerHeap>>,
}

#[derive(Clone)]
pub(super) struct WeakExec(Weak<hyper_executor>);

struct ExecWaker(AtomicBool);

/// An async task.
pub struct hyper_task {
    future: BoxFuture<BoxAny>,
    output: Option<BoxAny>,
    userdata: Userdata,
}

struct TaskFuture {
    task: Option<Box<hyper_task>>,
}

/// An async context for a task that contains the related waker.
pub struct hyper_context<'a>(Context<'a>);

/// A waker that is saved and used to waken a pending task.
pub struct hyper_waker {
    waker: std::task::Waker,
}

/// A descriptor for what type a `hyper_task` value is.
#[repr(C)]
pub enum hyper_task_return_type {
    /// The value of this task is null (does not imply an error).
    HYPER_TASK_EMPTY,
    /// The value of this task is `hyper_error *`.
    HYPER_TASK_ERROR,
    /// The value of this task is `hyper_clientconn *`.
    HYPER_TASK_CLIENTCONN,
    /// The value of this task is `hyper_response *`.
    HYPER_TASK_RESPONSE,
    /// The value of this task is `hyper_buf *`.
    HYPER_TASK_BUF,
    /// The value of this task is null (the task was a server-side connection task)
    HYPER_TASK_SERVERCONN,
}

pub(super) unsafe trait AsTaskType {
    fn as_task_type(&self) -> hyper_task_return_type;
}

pub(super) trait IntoDynTaskType {
    fn into_dyn_task_type(self) -> BoxAny;
}

// ===== impl hyper_executor =====

impl hyper_executor {
    fn new() -> Arc<hyper_executor> {
        Arc::new(hyper_executor {
            driver: Mutex::new(FuturesUnordered::new()),
            spawn_queue: Mutex::new(Vec::new()),
            is_woken: Arc::new(ExecWaker(AtomicBool::new(false))),
            timers: Arc::new(Mutex::new(crate::ffi::time::TimerHeap::new())),
        })
    }

    pub(super) fn downgrade(exec: &Arc<hyper_executor>) -> WeakExec {
        WeakExec(Arc::downgrade(exec))
    }

    pub(super) fn timer_heap(&self) -> &Arc<Mutex<crate::ffi::time::TimerHeap>> {
        &self.timers
    }

    fn spawn(&self, task: Box<hyper_task>) {
        self.spawn_queue
            .lock()
            .unwrap()
            .push(TaskFuture { task: Some(task) });
    }

    fn poll_next(&self) -> Option<Box<hyper_task>> {
        // Move any new tasks to the runnable queue
        self.drain_queue();

        // Wake all popped timers
        self.pop_timers();

        let waker = futures_util::task::waker_ref(&self.is_woken);
        let mut cx = Context::from_waker(&waker);

        loop {
            let poll = Pin::new(&mut *self.driver.lock().unwrap()).poll_next(&mut cx);
            match poll {
                Poll::Ready(val) => return val,
                Poll::Pending => {
                    // Time has progressed while polling above, so fire any wakers for timers that
                    // have popped in that window.
                    self.pop_timers();

                    // Check if any of the pending tasks tried to spawn some new tasks. If so,
                    // drain into the driver and loop.
                    if self.drain_queue() {
                        continue;
                    }

                    // If the driver called `wake` while we were polling or any timers have popped,
                    // we should poll again immediately!
                    if self.is_woken.0.swap(false, Ordering::SeqCst) {
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

    // Walk the timer heap waking active timers and discarding cancelled ones.
    fn pop_timers(&self) {
        let mut heap = self.timers.lock().unwrap();
        heap.process_timers();
    }
}

impl futures_util::task::ArcWake for ExecWaker {
    fn wake_by_ref(me: &Arc<ExecWaker>) {
        me.0.store(true, Ordering::SeqCst);
    }
}

// ===== impl WeakExec =====

impl WeakExec {
    pub(super) fn new() -> Self {
        WeakExec(Weak::new())
    }
}

impl<F> crate::rt::Executor<F> for WeakExec
where
    F: Future + Send + 'static,
    F::Output: Send + Sync + AsTaskType,
{
    fn execute(&self, fut: F) {
        if let Some(exec) = self.0.upgrade() {
            exec.spawn(hyper_task::boxed(fut));
        }
    }
}

ffi_fn! {
    /// Creates a new task executor.
    ///
    /// To avoid a memory leak, the executor must eventually be consumed by
    /// `hyper_executor_free`.
    fn hyper_executor_new() -> *const hyper_executor {
        Arc::into_raw(hyper_executor::new())
    } ?= ptr::null()
}

ffi_fn! {
    /// Frees an executor and any incomplete tasks still part of it.
    ///
    /// This should be used for any executor once it is no longer needed.
    fn hyper_executor_free(exec: *const hyper_executor) {
        drop(non_null!(Arc::from_raw(exec) ?= ()));
    }
}

ffi_fn! {
    /// Push a task onto the executor.
    ///
    /// The executor takes ownership of the task, which should not be accessed
    /// again unless returned back to the user with `hyper_executor_poll`.
    fn hyper_executor_push(exec: *const hyper_executor, task: *mut hyper_task) -> hyper_code {
        let exec = non_null!(&*exec ?= hyper_code::HYPERE_INVALID_ARG);
        let task = non_null!(Box::from_raw(task) ?= hyper_code::HYPERE_INVALID_ARG);
        exec.spawn(task);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    /// Polls the executor, trying to make progress on any tasks that have notified
    /// that they are ready again.
    ///
    /// If ready, returns a task from the executor that has completed.
    ///
    /// To avoid a memory leak, the task must eventually be consumed by
    /// `hyper_task_free`, or taken ownership of by `hyper_executor_push`
    /// without subsequently being given back by `hyper_executor_poll`.
    ///
    /// If there are no ready tasks, this returns `NULL`.
    fn hyper_executor_poll(exec: *const hyper_executor) -> *mut hyper_task {
        let exec = non_null!(&*exec ?= ptr::null_mut());
        match exec.poll_next() {
            Some(task) => Box::into_raw(task),
            None => ptr::null_mut(),
        }
    } ?= ptr::null_mut()
}

ffi_fn! {
    /// Returns the time until the executor will be able to make progress on tasks due to internal
    /// timers popping.  The executor should be polled soon after this time (if not earlier due to
    /// IO operations becoming available).
    ///
    /// Returns the time in milliseconds - a return value of -1 means there's no configured timers
    /// and the executor doesn't need polling until there's IO work available.
    fn hyper_executor_next_timer_pop(exec: *const hyper_executor) -> std::ffi::c_int {
        let exec = non_null!(&*exec ?= -1);
        match exec.timers.lock().unwrap().next_timer_pop() {
            Some(duration) => {
                let micros = duration.as_micros();
                ((micros + 999) / 1000) as _
            }
            None => -1
        }
    }
}

// ===== impl hyper_task =====

impl hyper_task {
    pub(super) fn boxed<F>(fut: F) -> Box<hyper_task>
    where
        F: Future + Send + 'static,
        F::Output: IntoDynTaskType + Send + Sync + 'static,
    {
        Box::new(hyper_task {
            future: Box::pin(async move { fut.await.into_dyn_task_type() }),
            output: None,
            userdata: Userdata::default(),
        })
    }

    fn output_type(&self) -> hyper_task_return_type {
        match self.output {
            None => hyper_task_return_type::HYPER_TASK_EMPTY,
            Some(ref val) => val.as_task_type(),
        }
    }
}

impl Future for TaskFuture {
    type Output = Box<hyper_task>;

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
    /// Free a task.
    ///
    /// This should only be used if the task isn't consumed by
    /// `hyper_clientconn_handshake` or taken ownership of by
    /// `hyper_executor_push`.
    fn hyper_task_free(task: *mut hyper_task) {
        drop(non_null!(Box::from_raw(task) ?= ()));
    }
}

ffi_fn! {
    /// Takes the output value of this task.
    ///
    /// This must only be called once polling the task on an executor has finished
    /// this task.
    ///
    /// Use `hyper_task_type` to determine the type of the `void *` return value.
    ///
    /// To avoid a memory leak, a non-empty return value must eventually be
    /// consumed by a function appropriate for its type, one of
    /// `hyper_error_free`, `hyper_clientconn_free`, `hyper_response_free`, or
    /// `hyper_buf_free`.
    fn hyper_task_value(task: *mut hyper_task) -> *mut c_void {
        let task = non_null!(&mut *task ?= ptr::null_mut());

        if let Some(val) = task.output.take() {
            let p = Box::into_raw(val) as *mut c_void;
            // protect from returning fake pointers to empty types
            if p == std::ptr::NonNull::<c_void>::dangling().as_ptr() {
                ptr::null_mut()
            } else {
                p
            }
        } else {
            ptr::null_mut()
        }
    } ?= ptr::null_mut()
}

ffi_fn! {
    /// Query the return type of this task.
    fn hyper_task_type(task: *mut hyper_task) -> hyper_task_return_type {
        // instead of blowing up spectacularly, just say this null task
        // doesn't have a value to retrieve.
        non_null!(&*task ?= hyper_task_return_type::HYPER_TASK_EMPTY).output_type()
    }
}

ffi_fn! {
    /// Set a user data pointer to be associated with this task.
    ///
    /// This value will be passed to task callbacks, and can be checked later
    /// with `hyper_task_userdata`.
    fn hyper_task_set_userdata(task: *mut hyper_task, userdata: *mut c_void, drop: hyper_userdata_drop) {
        let task = non_null!(&mut*task ?= ());
        task.userdata = Userdata::new(userdata, drop);
    }
}

ffi_fn! {
    /// Retrieve the userdata that has been set via `hyper_task_set_userdata`.
    fn hyper_task_userdata(task: *mut hyper_task) -> *mut c_void {
        non_null!(&*task ?= ptr::null_mut()).userdata.as_ptr()
    } ?= ptr::null_mut()
}

// ===== impl AsTaskType =====

unsafe impl AsTaskType for () {
    fn as_task_type(&self) -> hyper_task_return_type {
        hyper_task_return_type::HYPER_TASK_EMPTY
    }
}

unsafe impl AsTaskType for crate::Error {
    fn as_task_type(&self) -> hyper_task_return_type {
        hyper_task_return_type::HYPER_TASK_ERROR
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

impl<T> IntoDynTaskType for crate::Result<T>
where
    T: IntoDynTaskType + Send + Sync + 'static,
{
    fn into_dyn_task_type(self) -> BoxAny {
        match self {
            Ok(val) => val.into_dyn_task_type(),
            Err(err) => Box::new(err),
        }
    }
}

impl<T> IntoDynTaskType for Option<T>
where
    T: IntoDynTaskType + Send + Sync + 'static,
{
    fn into_dyn_task_type(self) -> BoxAny {
        match self {
            Some(val) => val.into_dyn_task_type(),
            None => ().into_dyn_task_type(),
        }
    }
}

// ===== impl hyper_context =====

impl hyper_context<'_> {
    pub(super) fn wrap<'a, 'b>(cx: &'a mut Context<'b>) -> &'a mut hyper_context<'b> {
        // A struct with only one field has the same layout as that field.
        unsafe { std::mem::transmute::<&mut Context<'_>, &mut hyper_context<'_>>(cx) }
    }
}

ffi_fn! {
    /// Copies a waker out of the task context.
    ///
    /// To avoid a memory leak, the waker must eventually be consumed by
    /// `hyper_waker_free` or `hyper_waker_wake`.
    fn hyper_context_waker(cx: *mut hyper_context<'_>) -> *mut hyper_waker {
        let waker = non_null!(&mut *cx ?= ptr::null_mut()).0.waker().clone();
        Box::into_raw(Box::new(hyper_waker { waker }))
    } ?= ptr::null_mut()
}

// ===== impl hyper_waker =====

ffi_fn! {
    /// Free a waker.
    ///
    /// This should only be used if the request isn't consumed by
    /// `hyper_waker_wake`.
    fn hyper_waker_free(waker: *mut hyper_waker) {
        drop(non_null!(Box::from_raw(waker) ?= ()));
    }
}

ffi_fn! {
    /// Wake up the task associated with a waker.
    ///
    /// NOTE: This consumes the waker. You should not use or free the waker afterwards.
    fn hyper_waker_wake(waker: *mut hyper_waker) {
        let waker = non_null!(Box::from_raw(waker) ?= ());
        waker.waker.wake();
    }
}
