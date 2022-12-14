//! Context switches tracking utilities.
//!
//! Those are mostly used for testing.

/// Returns the number of context switches of the calling fiber.
pub fn csw() -> i32 {
    static mut FUNCTION_DEFINED: bool = false;
    let lua = crate::lua_state();

    if unsafe { !FUNCTION_DEFINED } {
        #[rustfmt::skip]
        lua.exec(r#"
            function fiber_csw()
                local fiber = require('fiber')
                return fiber.info()[fiber.id()].csw
            end
        "#).unwrap();
        unsafe {
            FUNCTION_DEFINED = true;
        }
    }

    lua.get::<crate::tlua::LuaFunction<_>, _>("fiber_csw")
        .unwrap()
        .into_call()
        .unwrap()
}

/// Calls a function and checks whether it yielded.
///
/// It's mostly useful in tests.
///
/// See also: <https://www.tarantool.io/en/doc/latest/concepts/coop_multitasking/#app-yields>
///
/// # Examle
///
/// ```no_run
/// # use tarantool::fiber;
/// # use tarantool::fiber::check_yield;
/// # use tarantool::fiber::YieldResult::*;
/// # use std::time::Duration;
/// assert_eq!(
///     check_yield(|| fiber::sleep(Duration::ZERO)),
///     Yielded(())
/// );
/// ```
pub fn check_yield<F, T>(f: F) -> YieldResult<T>
where
    F: FnOnce() -> T,
{
    let csw_before = csw();
    let res = f();
    if csw() == csw_before {
        YieldResult::DidntYield(res)
    } else {
        YieldResult::Yielded(res)
    }
}

/// Possible [`check_yield`] results.
#[derive(Debug, PartialEq, Eq)]
pub enum YieldResult<T> {
    /// The function didn't yield.
    DidntYield(T),
    /// The function did yield.
    Yielded(T),
}