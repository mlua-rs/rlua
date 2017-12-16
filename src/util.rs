use std::{mem, process, ptr};
use std::sync::Arc;
use std::ffi::CStr;
use std::any::Any;
use std::os::raw::{c_char, c_int, c_void};
use std::panic::{catch_unwind, resume_unwind, UnwindSafe};

use ffi;
use error::{Error, Result};

// Checks that Lua has enough free stack space for future stack operations.
// On failure, this will clear the stack and panic.
pub unsafe fn check_stack(state: *mut ffi::lua_State, amount: c_int) {
    lua_assert!(
        state,
        ffi::lua_checkstack(state, amount) != 0,
        "out of stack space"
    );
}

// Run an operation on a lua_State and check that the stack change is what is
// expected.  If the stack change does not match, clears the stack and panics.
pub unsafe fn stack_guard<F, R>(state: *mut ffi::lua_State, change: c_int, op: F) -> R
where
    F: FnOnce() -> R,
{
    let expected = ffi::lua_gettop(state) + change;
    lua_assert!(
        state,
        expected >= 0,
        "too many stack values would be popped"
    );

    let res = op();

    let top = ffi::lua_gettop(state);
    lua_assert!(
        state,
        ffi::lua_gettop(state) == expected,
        "expected stack to be {}, got {}",
        expected,
        top
    );

    res
}

// Run an operation on a lua_State and automatically clean up the stack before
// returning.  Takes the lua_State, the expected stack size change, and an
// operation to run.  If the operation results in success, then the stack is
// inspected to make sure the change in stack size matches the expected change
// and otherwise this is a logic error and will panic.  If the operation results
// in an error, the stack is shrunk to the value before the call.  If the
// operation results in an error and the stack is smaller than the value before
// the call, then this is unrecoverable and this will panic.  If this function
// panics, it will clear the stack before panicking.
pub unsafe fn stack_err_guard<F, R>(state: *mut ffi::lua_State, change: c_int, op: F) -> Result<R>
where
    F: FnOnce() -> Result<R>,
{
    let expected = ffi::lua_gettop(state) + change;
    lua_assert!(
        state,
        expected >= 0,
        "too many stack values would be popped"
    );

    let res = op();

    let top = ffi::lua_gettop(state);
    if res.is_ok() {
        lua_assert!(
            state,
            ffi::lua_gettop(state) == expected,
            "expected stack to be {}, got {}",
            expected,
            top
        );
    } else {
        lua_assert!(
            state,
            top >= expected,
            "{} too many stack values popped",
            top - expected
        );
        if top > expected {
            ffi::lua_settop(state, expected);
        }
    }

    res
}

// Call a function that calls into the Lua API and may trigger a Lua error (longjmp) in a safe way.
// Wraps the inner function in a call to `lua_pcall`, so the inner function only has access to a
// limited lua stack.  `nargs` and `nresults` are similar to the parameters of `lua_pcall`, but the
// given function return type is not the return value count, instead the inner function return
// values are assumed to match the `nresults` param.  Internally uses 3 extra stack spaces, and does
// not call checkstack.
pub unsafe fn protect_lua_call<F, R>(
    state: *mut ffi::lua_State,
    nargs: c_int,
    nresults: c_int,
    f: F,
) -> Result<R>
where
    F: FnMut(*mut ffi::lua_State) -> R,
{
    struct Params<F, R> {
        function: F,
        ret: Option<R>,
        nresults: c_int,
    }

    unsafe extern "C" fn do_call<F, R>(state: *mut ffi::lua_State) -> c_int
    where
        F: FnMut(*mut ffi::lua_State) -> R,
    {
        let params = ffi::lua_touserdata(state, -1) as *mut Params<F, R>;
        ffi::lua_pop(state, 1);
        (*params).ret = Some(((*params).function)(state));
        if (*params).nresults == ffi::LUA_MULTRET {
            ffi::lua_gettop(state)
        } else {
            (*params).nresults
        }
    }

    let stack_start = ffi::lua_gettop(state) - nargs;

    ffi::lua_pushcfunction(state, error_traceback);
    ffi::lua_pushcfunction(state, do_call::<F, R>);
    ffi::lua_rotate(state, stack_start + 1, 2);

    let mut params = Params {
        function: f,
        ret: None,
        nresults,
    };

    ffi::lua_pushlightuserdata(state, &mut params as *mut Params<F, R> as *mut c_void);

    let ret = ffi::lua_pcall(state, nargs + 1, nresults, stack_start + 1);

    ffi::lua_remove(state, stack_start + 1);

    if ret == ffi::LUA_OK {
        Ok(params.ret.unwrap())
    } else {
        Err(pop_error(state, ret))
    }
}

// Pops an error off of the stack and returns it. If the error is actually a WrappedPanic, clears
// the current lua stack and continues the panic.  If the error on the top of the stack is actually
// a WrappedError, just returns it.  Otherwise, interprets the error as the appropriate lua error.
pub unsafe fn pop_error(state: *mut ffi::lua_State, err_code: c_int) -> Error {
    lua_assert!(
        state,
        err_code != ffi::LUA_OK && err_code != ffi::LUA_YIELD,
        "pop_error called with non-error return code"
    );

    if let Some(err) = pop_wrapped_error(state) {
        err
    } else if is_wrapped_panic(state, -1) {
        let panic = get_userdata::<WrappedPanic>(state, -1)
            .expect("WrappedPanic was somehow resurrected after garbage collection");
        if let Some(p) = (*panic).0.take() {
            ffi::lua_settop(state, 0);
            resume_unwind(p);
        } else {
            lua_panic!(state, "panic was resumed twice")
        }
    } else {
        let err_string = gc_guard(state, || {
            if let Some(s) = ffi::lua_tostring(state, -1).as_ref() {
                CStr::from_ptr(s).to_string_lossy().into_owned()
            } else {
                "<unprintable error>".to_owned()
            }
        });
        ffi::lua_pop(state, 1);

        match err_code {
            ffi::LUA_ERRRUN => Error::RuntimeError(err_string),
            ffi::LUA_ERRSYNTAX => {
                Error::SyntaxError {
                    // This seems terrible, but as far as I can tell, this is exactly what the
                    // stock Lua REPL does.
                    incomplete_input: err_string.ends_with("<eof>"),
                    message: err_string,
                }
            }
            ffi::LUA_ERRERR => {
                // The Lua manual documents this error wrongly: It is not raised when a message
                // handler errors, but rather when some specific situations regarding stack
                // overflow handling occurs. Since it is not very useful do differentiate
                // between that and "ordinary" runtime errors, we handle them the same way.
                Error::RuntimeError(err_string)
            }
            ffi::LUA_ERRMEM => {
                // This should be impossible, as we set the lua allocator to one that aborts
                // instead of failing.
                eprintln!("impossible Lua allocation error, aborting!");
                process::abort()
            }
            ffi::LUA_ERRGCMM => Error::GarbageCollectorError(err_string),
            _ => lua_panic!(state, "unrecognized lua error code"),
        }
    }
}

// Internally uses 4 stack spaces, does not call checkstack
pub unsafe fn push_string(state: *mut ffi::lua_State, s: &str) -> Result<()> {
    protect_lua_call(state, 0, 1, |state| {
        ffi::lua_pushlstring(state, s.as_ptr() as *const c_char, s.len());
    })
}

// Internally uses 4 stack spaces, does not call checkstack
pub unsafe fn push_userdata<T>(state: *mut ffi::lua_State, t: T) -> Result<()> {
    let mut t = Some(t);
    protect_lua_call(state, 0, 1, |state| {
        let ud = ffi::lua_newuserdata(state, mem::size_of::<Option<T>>()) as *mut Option<T>;
        ptr::write(ud, t.take());
    })
}

// Returns None in the case that the userdata has already been garbage collected.
pub unsafe fn get_userdata<T>(state: *mut ffi::lua_State, index: c_int) -> Result<*mut T> {
    let ud = ffi::lua_touserdata(state, index) as *mut Option<T>;
    lua_assert!(state, !ud.is_null(), "userdata pointer is null");
    (*ud)
        .as_mut()
        .map(|v| v as *mut T)
        .ok_or(Error::ExpiredUserData)
}

pub unsafe extern "C" fn userdata_destructor<T>(state: *mut ffi::lua_State) -> c_int {
    callback_error(state, || {
        *(ffi::lua_touserdata(state, 1) as *mut Option<T>) = None;
        Ok(0)
    })
}

// In the context of a lua callback, this will call the given function and if the given function
// returns an error, *or if the given function panics*, this will result in a call to lua_error (a
// longjmp).  The error or panic is wrapped in such a way that when calling pop_error back on
// the rust side, it will resume the panic.
pub unsafe fn callback_error<R, F>(state: *mut ffi::lua_State, f: F) -> R
where
    F: FnOnce() -> Result<R> + UnwindSafe,
{
    match catch_unwind(f) {
        Ok(Ok(r)) => r,
        Ok(Err(err)) => {
            push_wrapped_error(state, err);
            ffi::lua_error(state)
        }
        Err(p) => {
            push_wrapped_panic(state, p);
            ffi::lua_error(state)
        }
    }
}

// Takes an error at the top of the stack, and if it is a WrappedError, converts it to an
// Error::CallbackError with a traceback, if it is some lua type, prints the error along with a
// traceback, and if it is a WrappedPanic, does not modify it.
pub unsafe extern "C" fn error_traceback(state: *mut ffi::lua_State) -> c_int {
    if let Some(error) = pop_wrapped_error(state) {
        ffi::luaL_traceback(state, state, ptr::null(), 0);
        let traceback = CStr::from_ptr(ffi::lua_tostring(state, -1))
            .to_string_lossy()
            .into_owned();
        push_wrapped_error(
            state,
            Error::CallbackError {
                traceback,
                cause: Arc::new(error),
            },
        );
        ffi::lua_remove(state, -2);
    } else if !is_wrapped_panic(state, 1) {
        let s = ffi::lua_tostring(state, 1);
        let s = if s.is_null() {
            cstr!("<unprintable Rust panic>")
        } else {
            s
        };
        ffi::luaL_traceback(state, state, s, 0);
        ffi::lua_remove(state, -2);
    }
    1
}

// A variant of pcall that does not allow lua to catch panic errors from callback_error
pub unsafe extern "C" fn safe_pcall(state: *mut ffi::lua_State) -> c_int {
    let top = ffi::lua_gettop(state);
    if top == 0 {
        ffi::lua_pushstring(state, cstr!("not enough arguments to pcall"));
        ffi::lua_error(state);
    } else if ffi::lua_pcall(state, top - 1, ffi::LUA_MULTRET, 0) != ffi::LUA_OK {
        if is_wrapped_panic(state, -1) {
            ffi::lua_error(state);
        }
        ffi::lua_pushboolean(state, 0);
        ffi::lua_insert(state, -2);
        2
    } else {
        ffi::lua_pushboolean(state, 1);
        ffi::lua_insert(state, 1);
        ffi::lua_gettop(state)
    }
}

// A variant of xpcall that does not allow lua to catch panic errors from callback_error
pub unsafe extern "C" fn safe_xpcall(state: *mut ffi::lua_State) -> c_int {
    unsafe extern "C" fn xpcall_msgh(state: *mut ffi::lua_State) -> c_int {
        if is_wrapped_panic(state, -1) {
            1
        } else {
            ffi::lua_pushvalue(state, ffi::lua_upvalueindex(1));
            ffi::lua_insert(state, 1);
            ffi::lua_call(state, ffi::lua_gettop(state) - 1, ffi::LUA_MULTRET);
            ffi::lua_gettop(state)
        }
    }

    let top = ffi::lua_gettop(state);
    if top < 2 {
        ffi::lua_pushstring(state, cstr!("not enough arguments to xpcall"));
        ffi::lua_error(state);
    }

    ffi::lua_pushvalue(state, 2);
    ffi::lua_pushcclosure(state, xpcall_msgh, 1);
    ffi::lua_copy(state, 1, 2);
    ffi::lua_replace(state, 1);

    let res = ffi::lua_pcall(state, ffi::lua_gettop(state) - 2, ffi::LUA_MULTRET, 1);
    if res != ffi::LUA_OK {
        if is_wrapped_panic(state, -1) {
            ffi::lua_error(state);
        }
        ffi::lua_pushboolean(state, 0);
        ffi::lua_insert(state, -2);
        2
    } else {
        ffi::lua_pushboolean(state, 1);
        ffi::lua_insert(state, 2);
        ffi::lua_gettop(state) - 1
    }
}

// Does not call checkstack, uses 1 stack space
pub unsafe fn main_state(state: *mut ffi::lua_State) -> *mut ffi::lua_State {
    ffi::lua_rawgeti(state, ffi::LUA_REGISTRYINDEX, ffi::LUA_RIDX_MAINTHREAD);
    let main_state = ffi::lua_tothread(state, -1);
    ffi::lua_pop(state, 1);
    main_state
}

// Pushes a WrappedError::Error to the top of the stack
pub unsafe fn push_wrapped_error(state: *mut ffi::lua_State, err: Error) {
    ffi::luaL_checkstack(state, 2, ptr::null());

    gc_guard(state, || {
        let ud = ffi::lua_newuserdata(state, mem::size_of::<Option<WrappedError>>())
            as *mut Option<WrappedError>;
        ptr::write(ud, Some(WrappedError(err)))
    });

    get_error_metatable(state);
    ffi::lua_setmetatable(state, -2);
}

// Pops a WrappedError off of the top of the stack, if it is a WrappedError.  If
// it is not a WrappedError, returns None and does not pop anything.
pub unsafe fn pop_wrapped_error(state: *mut ffi::lua_State) -> Option<Error> {
    if ffi::lua_gettop(state) == 0 || !is_wrapped_error(state, -1) {
        None
    } else {
        let err = &*get_userdata::<WrappedError>(state, -1)
            .expect("WrappedError was somehow resurrected after garbage collection");
        let err = err.0.clone();
        ffi::lua_pop(state, 1);
        Some(err)
    }
}

struct WrappedError(pub Error);
struct WrappedPanic(pub Option<Box<Any + Send>>);

// Pushes a WrappedError::Panic to the top of the stack
unsafe fn push_wrapped_panic(state: *mut ffi::lua_State, panic: Box<Any + Send>) {
    ffi::luaL_checkstack(state, 2, ptr::null());

    gc_guard(state, || {
        let ud = ffi::lua_newuserdata(state, mem::size_of::<Option<WrappedPanic>>())
            as *mut Option<WrappedPanic>;
        ptr::write(ud, Some(WrappedPanic(Some(panic))))
    });

    get_panic_metatable(state);
    ffi::lua_setmetatable(state, -2);
}

// Checks if the value at the given index is a WrappedError
unsafe fn is_wrapped_error(state: *mut ffi::lua_State, index: c_int) -> bool {
    assert_ne!(
        ffi::lua_checkstack(state, 2),
        0,
        "somehow not enough stack space to check if a value is a WrappedError"
    );

    let index = ffi::lua_absindex(state, index);

    let userdata = ffi::lua_touserdata(state, index);
    if userdata.is_null() {
        return false;
    }

    if ffi::lua_getmetatable(state, index) == 0 {
        return false;
    }

    get_error_metatable(state);
    let res = ffi::lua_rawequal(state, -1, -2) != 0;
    ffi::lua_pop(state, 2);
    res
}

// Checks if the value at the given index is a WrappedPanic
unsafe fn is_wrapped_panic(state: *mut ffi::lua_State, index: c_int) -> bool {
    assert_ne!(
        ffi::lua_checkstack(state, 2),
        0,
        "somehow not enough stack space to check if a value is a wrapped panic"
    );

    let index = ffi::lua_absindex(state, index);

    let userdata = ffi::lua_touserdata(state, index);
    if userdata.is_null() {
        return false;
    }

    if ffi::lua_getmetatable(state, index) == 0 {
        return false;
    }

    get_panic_metatable(state);
    let res = ffi::lua_rawequal(state, -1, -2) != 0;
    ffi::lua_pop(state, 2);
    res
}

unsafe fn get_error_metatable(state: *mut ffi::lua_State) -> c_int {
    static ERROR_METATABLE_REGISTRY_KEY: u8 = 0;

    unsafe extern "C" fn error_tostring(state: *mut ffi::lua_State) -> c_int {
        callback_error(state, || {
            if is_wrapped_error(state, -1) {
                let error = get_userdata::<WrappedError>(state, -1)
                    .expect("WrappedError was somehow resurrected after garbage collection");
                let error_str = (*error).0.to_string();
                gc_guard(state, || {
                    ffi::lua_pushlstring(
                        state,
                        error_str.as_ptr() as *const c_char,
                        error_str.len(),
                    )
                });
                ffi::lua_remove(state, -2);

                Ok(1)
            } else {
                panic!("userdata mismatch in Error metamethod");
            }
        })
    }

    ffi::lua_pushlightuserdata(
        state,
        &ERROR_METATABLE_REGISTRY_KEY as *const u8 as *mut c_void,
    );
    let t = ffi::lua_gettable(state, ffi::LUA_REGISTRYINDEX);

    if t != ffi::LUA_TTABLE {
        ffi::lua_pop(state, 1);

        ffi::luaL_checkstack(state, 8, ptr::null());

        gc_guard(state, || {
            ffi::lua_newtable(state);
            ffi::lua_pushlightuserdata(
                state,
                &ERROR_METATABLE_REGISTRY_KEY as *const u8 as *mut c_void,
            );
            ffi::lua_pushvalue(state, -2);

            ffi::lua_pushstring(state, cstr!("__gc"));
            ffi::lua_pushcfunction(state, userdata_destructor::<WrappedError>);
            ffi::lua_rawset(state, -3);

            ffi::lua_pushstring(state, cstr!("__tostring"));
            ffi::lua_pushcfunction(state, error_tostring);
            ffi::lua_rawset(state, -3);

            ffi::lua_pushstring(state, cstr!("__metatable"));
            ffi::lua_pushboolean(state, 0);
            ffi::lua_rawset(state, -3);

            ffi::lua_rawset(state, ffi::LUA_REGISTRYINDEX);
        })
    }

    ffi::LUA_TTABLE
}

unsafe fn get_panic_metatable(state: *mut ffi::lua_State) -> c_int {
    static PANIC_METATABLE_REGISTRY_KEY: u8 = 0;

    ffi::lua_pushlightuserdata(
        state,
        &PANIC_METATABLE_REGISTRY_KEY as *const u8 as *mut c_void,
    );
    let t = ffi::lua_gettable(state, ffi::LUA_REGISTRYINDEX);

    if t != ffi::LUA_TTABLE {
        ffi::lua_pop(state, 1);

        ffi::luaL_checkstack(state, 8, ptr::null());

        gc_guard(state, || {
            ffi::lua_newtable(state);
            ffi::lua_pushlightuserdata(
                state,
                &PANIC_METATABLE_REGISTRY_KEY as *const u8 as *mut c_void,
            );
            ffi::lua_pushvalue(state, -2);

            ffi::lua_pushstring(state, cstr!("__gc"));
            ffi::lua_pushcfunction(state, userdata_destructor::<WrappedPanic>);
            ffi::lua_rawset(state, -3);

            ffi::lua_pushstring(state, cstr!("__metatable"));
            ffi::lua_pushboolean(state, 0);
            ffi::lua_rawset(state, -3);

            ffi::lua_rawset(state, ffi::LUA_REGISTRYINDEX);
        });
    }

    ffi::LUA_TTABLE
}

// Runs the given function with the Lua garbage collector disabled.  `rlua` assumes that all
// allocation failures are aborts, so when the garbage collector is disabled, 'm' functions that can
// cause either an allocation error or a a `__gc` metamethod error are prevented from causing errors
// at all.  The given function should never panic or longjmp, because this could inadverntently
// disable the gc.  This is useful when error handling must allocate, and `__gc` errors at that time
// would shadow more important errors, or be extremely difficult to handle safely.
unsafe fn gc_guard<R, F: FnOnce() -> R>(state: *mut ffi::lua_State, f: F) -> R {
    if ffi::lua_gc(state, ffi::LUA_GCISRUNNING, 0) != 0 {
        ffi::lua_gc(state, ffi::LUA_GCSTOP, 0);
        let r = f();
        ffi::lua_gc(state, ffi::LUA_GCRESTART, 0);
        r
    } else {
        f()
    }
}
