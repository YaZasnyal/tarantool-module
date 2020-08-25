use std::io;
use std::os::raw::{c_int, c_schar};

use luajit::ffi::{lua_pushinteger, lua_State, luaL_error};
use tester::{
    ColorConfig, Options, OutputFormat, run_tests_console, RunIgnored, ShouldPanic, TestDesc, TestDescAndFn,
    TestFn, TestName, TestOpts, TestType
};

mod test_fiber;
mod test_box;

fn add_test_default(name: &'static str, f: fn()) -> TestDescAndFn {
    TestDescAndFn{
        desc: TestDesc{
            name: TestName::StaticTestName(name),
            ignore: false,
            should_panic: ShouldPanic::No,
            allow_fail: false,
            test_type: TestType::UnitTest
        },
        testfn: TestFn::StaticTestFn(f)
    }
}

fn run() -> Result<bool, io::Error>{
    let opts = TestOpts{
        list: false,
        filter: None,
        filter_exact: false,
        force_run_in_process: false,
        exclude_should_panic: false,
        run_ignored: RunIgnored::No,
        run_tests: true,
        bench_benchmarks: false,
        logfile: None,
        nocapture: false,
        color: ColorConfig::AutoColor,
        format: OutputFormat::Pretty,
        test_threads: Some(1),
        skip: vec![],
        time_options: None,
        options: Options::new()
    };

    let tests = vec![
        add_test_default("fiber", test_fiber::test_fiber),
        add_test_default("fiber_arg", test_fiber::test_fiber_arg),
        add_test_default("fiber_cancel", test_fiber::test_fiber_cancel),
        add_test_default("fiber_wake", test_fiber::test_fiber_wake),

        add_test_default("box_space_get_by_name", test_box::test_space_get_by_name),
        add_test_default("box_index_get_by_name", test_box::test_index_get_by_name),
        add_test_default("box_insert", test_box::test_box_insert),
        add_test_default("box_replace", test_box::test_box_replace),
        add_test_default("box_delete", test_box::test_box_delete),
        add_test_default("box_update", test_box::test_box_update),
        add_test_default("box_upsert", test_box::test_box_upsert),
        add_test_default("box_truncate", test_box::test_box_truncate),
        add_test_default("box_get", test_box::test_box_get),
        add_test_default("box_select", test_box::test_box_select),
        add_test_default("box_len", test_box::test_box_len),
        add_test_default("box_random", test_box::test_box_random),
        add_test_default("box_min_max", test_box::test_box_min_max),
        add_test_default("box_count", test_box::test_box_count),
        add_test_default("box_extract_key", test_box::test_box_extract_key),
    ];

    run_tests_console(&opts, tests)
}

#[no_mangle]
pub extern "C" fn luaopen_libtarantool_module_test_runner(l: *mut lua_State) -> c_int {
    match run() {
        Ok(is_success) => {
            unsafe { lua_pushinteger(l, (!is_success) as isize) };
            1
        }
        Err(e) => {
            unsafe { luaL_error(l, e.to_string().as_ptr() as *const c_schar) };
            0
        }
    }
}