use std::ffi::CString;
use std::marker::PhantomData;
use std::os::raw::c_void;

use va_list::VaList;

use crate::error::{Error, TarantoolError};

/// Contains information about fiber
pub struct Fiber<'a, T: 'a> {
    inner: *mut ffi::Fiber,
    callback: *mut c_void,
    phantom: PhantomData<&'a T>,
}

impl<'a, T> Fiber<'a, T> {
    /// Create a new fiber.
    ///
    /// Takes a fiber from fiber cache, if it's not empty. Can fail only if there is not enough memory for
    /// the fiber structure or fiber stack.
    ///
    /// The created fiber automatically returns itself to the fiber cache when its `main` function
    /// completes.
    ///
    /// - `name` - string with fiber name
    /// - `callback` - function for run inside fiber
    ///
    /// See also: [start](#method.start)
    pub fn new<F>(name: &str, callback: &mut F) -> Self
    where
        F: FnMut(Box<T>) -> i32,
    {
        let (callback_ptr, trampoline) = unsafe { unpack_callback(callback) };
        Self {
            inner: unsafe { ffi::fiber_new(CString::new(name).unwrap().as_ptr(), trampoline) },
            callback: callback_ptr,
            phantom: PhantomData,
        }
    }

    /// Create a new fiber with defined attributes.
    ///
    /// Can fail only if there is not enough memory for the fiber structure or fiber stack.
    ///
    /// The created fiber automatically returns itself to the fiber cache if has default stack size
    /// when its `main` function completes.
    ///
    /// - `name` - string with fiber name
    /// - `fiber_attr` - fiber attributes
    /// - `callback` - function for run inside fiber
    ///
    /// See also: [start](#method.start)
    pub fn new_with_attr<F>(name: &str, fiber_attr: &FiberAttr, callback: &mut F) -> Self
    where
        F: FnMut(Box<T>) -> i32,
    {
        let (callback_ptr, trampoline) = unsafe { unpack_callback(callback) };
        Self {
            inner: unsafe {
                ffi::fiber_new_ex(
                    CString::new(name).unwrap().as_ptr(),
                    fiber_attr.inner,
                    trampoline,
                )
            },
            callback: callback_ptr,
            phantom: PhantomData,
        }
    }

    /// Start execution of created fiber.
    ///
    /// - `arg` - argument to start the fiber with
    ///
    /// See also: [new](#method.new)
    pub fn start(&mut self, arg: T) {
        unsafe {
            ffi::fiber_start(self.inner, self.callback, Box::into_raw(Box::<T>::new(arg)));
        }
    }

    /// Interrupt a synchronous wait of a fiber
    pub fn wakeup(&self) {
        unsafe { ffi::fiber_wakeup(self.inner) }
    }

    /// Wait until the fiber is dead and then move its execution status to the caller.
    /// The fiber must not be detached (See also: [set_joinable](#method.set_joinable)).
    ///
    /// Returns fiber function ret code
    pub fn join(&self) -> i32 {
        unsafe { ffi::fiber_join(self.inner) }
    }

    /// Set fiber to be joinable (false by default).
    ///
    /// - `is_joinable` - status to set
    pub fn set_joinable(&mut self, is_joinable: bool) {
        unsafe { ffi::fiber_set_joinable(self.inner, is_joinable) }
    }

    /// Cancel the subject fiber. (set `FIBER_IS_CANCELLED` flag)
    ///
    /// If target fiber's flag `FIBER_IS_CANCELLABLE` set, then it would be woken up (maybe prematurely).
    /// Then current fiber yields until the target fiber is dead (or is woken up by
    /// [wakeup](#method.wakeup)).
    pub fn cancel(&mut self) {
        unsafe { ffi::fiber_cancel(self.inner) }
    }
}

/// Make it possible or not possible to wakeup the current
/// fiber immediately when it's cancelled.
///
/// - `is_cancellable` - status to set
///
/// Returns previous state.
pub fn set_cancellable(is_cancellable: bool) -> bool {
    unsafe { ffi::fiber_set_cancellable(is_cancellable) }
}

/// Check current fiber for cancellation (it must be checked manually).
pub fn is_cancelled() -> bool {
    unsafe { ffi::fiber_is_cancelled() }
}

/// Put the current fiber to sleep for at least `time` seconds.
///
/// - `time` - time to sleep
///
/// **Note:** this is a cancellation point (See also: [is_cancelled](fn.is_cancelled.html))
pub fn sleep(time: f64) {
    unsafe { ffi::fiber_sleep(time) }
}

/// Report loop begin time as double (cheap).
pub fn time() -> f64 {
    unsafe { ffi::fiber_time() }
}

/// Report loop begin time as 64-bit int.
pub fn time64() -> u64 {
    unsafe { ffi::fiber_time64() }
}

/// Report loop begin time as double (cheap). Uses monotonic clock.
pub fn clock() -> f64 {
    unsafe { ffi::fiber_clock() }
}

/// Report loop begin time as 64-bit int. Uses monotonic clock.
pub fn clock64() -> u64 {
    unsafe { ffi::fiber_clock64() }
}

/// Return control to another fiber and wait until it'll be woken.
///
/// See also: [fiber_wakeup](struct.Fiber.html#method.wakeup)
pub fn fiber_yield() {
    unsafe { ffi::fiber_yield() }
}

/// Reschedule fiber to end of event loop cycle.
pub fn fiber_reschedule() {
    unsafe { ffi::fiber_reschedule() }
}

/// Fiber attributes container
pub struct FiberAttr {
    inner: *mut ffi::FiberAttr,
}

impl FiberAttr {
    /// Create a new fiber attribute container and initialize it with default parameters.
    /// Can be used for many fibers creation, corresponding fibers will not take ownership.
    ///
    /// This is safe to drop `FiberAttr` value when fibers created with this attribute still exist.
    pub fn new() -> Self {
        FiberAttr {
            inner: unsafe { ffi::fiber_attr_new() },
        }
    }

    /// Get stack size from the fiber attribute.
    ///
    /// Returns: stack size
    pub fn stack_size(&self) -> usize {
        unsafe { ffi::fiber_attr_getstacksize(self.inner) }
    }

    ///Set stack size for the fiber attribute.
    ///
    /// - `stack_size` - stack size for new fibers
    pub fn set_stack_size(&mut self, stack_size: usize) -> Result<(), Error> {
        if unsafe { ffi::fiber_attr_setstacksize(self.inner, stack_size) } < 0 {
            Err(TarantoolError::last().into())
        } else {
            Ok(())
        }
    }
}

impl Drop for FiberAttr {
    fn drop(&mut self) {
        unsafe { ffi::fiber_attr_delete(self.inner) }
    }
}

/// Conditional variable for cooperative multitasking (fibers).
///
/// A cond (short for "condition variable") is a synchronization primitive
/// that allow fibers to yield until some predicate is satisfied. Fiber
/// conditions have two basic operations - `wait()` and `signal()`. [wait()](#method.wait)
/// suspends execution of fiber (i.e. yields) until [signal()](#method.signal) is called.
///
/// Unlike `pthread_cond`, `fiber_cond` doesn't require mutex/latch wrapping.
pub struct FiberCond {
    inner: *mut ffi::FiberCond,
}

impl FiberCond {
    /// Instantiate a new fiber cond object.
    pub fn new() -> Self {
        FiberCond {
            inner: unsafe { ffi::fiber_cond_new() },
        }
    }

    /// Wake one fiber waiting for the cond.
    /// Does nothing if no one is waiting.
    pub fn signal(&self) {
        unsafe { ffi::fiber_cond_signal(self.inner) }
    }

    /// Wake up all fibers waiting for the cond.
    pub fn broadcast(&self) {
        unsafe { ffi::fiber_cond_broadcast(self.inner) }
    }

    /// Suspend the execution of the current fiber (i.e. yield) until [signal()](#method.signal) is called.
    ///
    /// Like pthread_cond, FiberCond can issue spurious wake ups caused by explicit
    /// [Fiber::wakeup()](struct.Fiber.html#method.wakeup) or [Fiber::cancel()](struct.Fiber.html#method.cancel)
    /// calls. It is highly recommended to wrap calls to this function into a loop
    /// and check an actual predicate and `fiber_testcancel()` on every iteration.
    ///
    /// - `timeout` - timeout in seconds
    ///
    /// Returns:
    /// - `true` on [signal()](#method.signal) call or a spurious wake up.
    /// - `false` on timeout, diag is set to `TimedOut`
    pub fn wait_timeout(&self, timeout: f64) -> bool {
        !(unsafe { ffi::fiber_cond_wait_timeout(self.inner, timeout) } < 0)
    }

    /// Shortcut for [wait_timeout()](#method.wait_timeout).
    pub fn wait(&self) -> bool {
        !(unsafe { ffi::fiber_cond_wait(self.inner) } < 0)
    }
}

impl Drop for FiberCond {
    fn drop(&mut self) {
        unsafe { ffi::fiber_cond_delete(self.inner) }
    }
}

pub(crate) mod ffi {
    use std::os::raw::{c_char, c_int};

    use va_list::VaList;

    #[repr(C)]
    pub struct Fiber {
        _unused: [u8; 0],
    }

    pub type FiberFunc = Option<unsafe extern "C" fn(VaList) -> c_int>;

    extern "C" {
        pub fn fiber_new(name: *const c_char, f: FiberFunc) -> *mut Fiber;
        pub fn fiber_new_ex(
            name: *const c_char,
            fiber_attr: *const FiberAttr,
            f: FiberFunc,
        ) -> *mut Fiber;
        pub fn fiber_yield();
        pub fn fiber_start(callee: *mut Fiber, ...);
        pub fn fiber_wakeup(f: *mut Fiber);
        pub fn fiber_cancel(f: *mut Fiber);
        pub fn fiber_set_cancellable(yesno: bool) -> bool;
        pub fn fiber_set_joinable(fiber: *mut Fiber, yesno: bool);
        pub fn fiber_join(f: *mut Fiber) -> c_int;
        pub fn fiber_sleep(s: f64);
        pub fn fiber_is_cancelled() -> bool;
        pub fn fiber_time() -> f64;
        pub fn fiber_time64() -> u64;
        pub fn fiber_clock() -> f64;
        pub fn fiber_clock64() -> u64;
        pub fn fiber_reschedule();
    }

    #[repr(C)]
    pub struct FiberAttr {
        _unused: [u8; 0],
    }

    extern "C" {
        pub fn fiber_attr_new() -> *mut FiberAttr;
        pub fn fiber_attr_delete(fiber_attr: *mut FiberAttr);
        pub fn fiber_attr_setstacksize(fiber_attr: *mut FiberAttr, stack_size: usize) -> c_int;
        pub fn fiber_attr_getstacksize(fiber_attr: *mut FiberAttr) -> usize;
    }

    #[repr(C)]
    pub struct FiberCond {
        _unused: [u8; 0],
    }

    extern "C" {
        pub fn fiber_cond_new() -> *mut FiberCond;
        pub fn fiber_cond_delete(cond: *mut FiberCond);
        pub fn fiber_cond_signal(cond: *mut FiberCond);
        pub fn fiber_cond_broadcast(cond: *mut FiberCond);
        pub fn fiber_cond_wait_timeout(cond: *mut FiberCond, timeout: f64) -> c_int;
        pub fn fiber_cond_wait(cond: *mut FiberCond) -> c_int;
    }
}

pub(crate) unsafe fn unpack_callback<F, T>(callback: &mut F) -> (*mut c_void, ffi::FiberFunc)
where
    F: FnMut(Box<T>) -> i32,
{
    unsafe extern "C" fn trampoline<F, T>(mut args: VaList) -> i32
    where
        F: FnMut(Box<T>) -> i32,
    {
        let closure: &mut F = &mut *(args.get::<*const c_void>() as *mut F);
        let arg = Box::from_raw(args.get::<*const c_void>() as *mut T);
        (*closure)(arg)
    }
    (callback as *mut F as *mut c_void, Some(trampoline::<F, T>))
}
