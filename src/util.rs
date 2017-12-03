use std::mem;
use std::ptr;
use std::process;
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
        "internal stack error: too many values would be popped"
    );

    let res = op();

    let top = ffi::lua_gettop(state);
    lua_assert!(
        state,
        ffi::lua_gettop(state) == expected,
        "internal stack error: expected stack to be {}, got {}",
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
        "internal stack error: too many values would be popped"
    );

    let res = op();

    let top = ffi::lua_gettop(state);
    if res.is_ok() {
        lua_assert!(
            state,
            ffi::lua_gettop(state) == expected,
            "internal stack error: expected stack to be {}, got {}",
            expected,
            top
        );
    } else {
        lua_assert!(
            state,
            top >= expected,
            "internal stack error: {} too many values popped",
            top - expected
        );
        if top > expected {
            ffi::lua_settop(state, expected);
        }
    }

    res
}

// Protected version of lua_gettable, uses 3 stack spaces, does not call checkstack.
pub unsafe fn pgettable(state: *mut ffi::lua_State, index: c_int) -> Result<c_int> {
    unsafe extern "C" fn gettable(state: *mut ffi::lua_State) -> c_int {
        ffi::lua_gettable(state, -2);
        1
    }

    let table_index = ffi::lua_absindex(state, index);

    ffi::lua_pushcfunction(state, gettable);
    ffi::lua_pushvalue(state, table_index);
    ffi::lua_pushvalue(state, -3);
    ffi::lua_remove(state, -4);

    handle_error(state, pcall_with_traceback(state, 2, 1))?;
    Ok(ffi::lua_type(state, -1))
}

// Protected version of lua_settable, uses 4 stack spaces, does not call checkstack.
pub unsafe fn psettable(state: *mut ffi::lua_State, index: c_int) -> Result<()> {
    unsafe extern "C" fn settable(state: *mut ffi::lua_State) -> c_int {
        ffi::lua_settable(state, -3);
        0
    }

    let table_index = ffi::lua_absindex(state, index);

    ffi::lua_pushcfunction(state, settable);
    ffi::lua_pushvalue(state, table_index);
    ffi::lua_pushvalue(state, -4);
    ffi::lua_pushvalue(state, -4);
    ffi::lua_remove(state, -5);
    ffi::lua_remove(state, -5);

    handle_error(state, pcall_with_traceback(state, 3, 0))?;
    Ok(())
}

// Protected version of luaL_len, uses 2 stack spaces, does not call checkstack.
pub unsafe fn plen(state: *mut ffi::lua_State, index: c_int) -> Result<ffi::lua_Integer> {
    unsafe extern "C" fn len(state: *mut ffi::lua_State) -> c_int {
        ffi::lua_pushinteger(state, ffi::luaL_len(state, -1));
        1
    }

    let table_index = ffi::lua_absindex(state, index);

    ffi::lua_pushcfunction(state, len);
    ffi::lua_pushvalue(state, table_index);

    handle_error(state, pcall_with_traceback(state, 1, 1))?;
    let len = ffi::lua_tointeger(state, -1);
    ffi::lua_pop(state, 1);
    Ok(len)
}

// Protected version of lua_geti, uses 3 stack spaces, does not call checkstack.
pub unsafe fn pgeti(
    state: *mut ffi::lua_State,
    index: c_int,
    i: ffi::lua_Integer,
) -> Result<c_int> {
    unsafe extern "C" fn geti(state: *mut ffi::lua_State) -> c_int {
        let i = ffi::lua_tointeger(state, -1);
        ffi::lua_geti(state, -2, i);
        1
    }

    let table_index = ffi::lua_absindex(state, index);

    ffi::lua_pushcfunction(state, geti);
    ffi::lua_pushvalue(state, table_index);
    ffi::lua_pushinteger(state, i);

    handle_error(state, pcall_with_traceback(state, 2, 1))?;
    Ok(ffi::lua_type(state, -1))
}

// Protected version of lua_next, uses 3 stack spaces, does not call checkstack.
pub unsafe fn pnext(state: *mut ffi::lua_State, index: c_int) -> Result<c_int> {
    unsafe extern "C" fn next(state: *mut ffi::lua_State) -> c_int {
        if ffi::lua_next(state, -2) == 0 {
            0
        } else {
            2
        }
    }

    let table_index = ffi::lua_absindex(state, index);

    ffi::lua_pushcfunction(state, next);
    ffi::lua_pushvalue(state, table_index);
    ffi::lua_pushvalue(state, -3);
    ffi::lua_remove(state, -4);

    let stack_start = ffi::lua_gettop(state) - 3;
    handle_error(state, pcall_with_traceback(state, 2, ffi::LUA_MULTRET))?;
    let nresults = ffi::lua_gettop(state) - stack_start;
    if nresults == 0 {
        Ok(0)
    } else {
        Ok(1)
    }
}

// If the return code indicates an error, pops the error off of the stack and
// returns Err. If the error is actually a WrappedPanic, clears the current lua
// stack and continues the panic.  If the error on the top of the stack is
// actually a WrappedError, just returns it.  Otherwise, interprets the error as
// the appropriate lua error.
pub unsafe fn handle_error(state: *mut ffi::lua_State, err: c_int) -> Result<()> {
    if err == ffi::LUA_OK || err == ffi::LUA_YIELD {
        Ok(())
    } else if let Some(err) = pop_wrapped_error(state) {
        Err(err)
    } else if is_wrapped_panic(state, -1) {
        let panic = get_userdata::<WrappedPanic>(state, -1);
        if let Some(p) = (*panic).0.take() {
            ffi::lua_settop(state, 0);
            resume_unwind(p);
        } else {
            lua_panic!(state, "internal error: panic was resumed twice")
        }
    } else {
        let err_string = if let Some(s) = ffi::lua_tolstring(state, -1, ptr::null_mut()).as_ref() {
            CStr::from_ptr(s)
                .to_str()
                .unwrap_or_else(|_| "<unprintable error>")
                .to_owned()
        } else {
            "<unprintable error>".to_owned()
        };
        ffi::lua_pop(state, 1);

        Err(match err {
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
                eprintln!("Lua memory error, aborting!");
                process::abort()
            }
            ffi::LUA_ERRGCMM => {
                // This should be impossible, since we wrap setmetatable to protect __gc
                // metamethods, but if we do end up here then the same logic as setmetatable
                // applies and we must abort.
                eprintln!("Lua error during __gc, aborting!");
                process::abort()
            }
            _ => lua_panic!(state, "internal error: unrecognized lua error code"),
        })
    }
}

pub unsafe fn push_string(state: *mut ffi::lua_State, s: &str) {
    ffi::lua_pushlstring(state, s.as_ptr() as *const c_char, s.len());
}

pub unsafe fn push_userdata<T>(state: *mut ffi::lua_State, t: T) {
    let ud = ffi::lua_newuserdata(state, mem::size_of::<Option<T>>()) as *mut Option<T>;
    ptr::write(ud, Some(t));
}

pub unsafe fn get_userdata<T>(state: *mut ffi::lua_State, index: c_int) -> *mut T {
    let ud = ffi::lua_touserdata(state, index) as *mut Option<T>;
    lua_assert!(state, !ud.is_null());
    lua_assert!(state, (*ud).is_some(), "access of expired userdata");
    (*ud).as_mut().unwrap()
}

pub unsafe extern "C" fn userdata_destructor<T>(state: *mut ffi::lua_State) -> c_int {
    match catch_unwind(|| {
        *(ffi::lua_touserdata(state, 1) as *mut Option<T>) = None;
        0
    }) {
        Ok(r) => r,
        Err(p) => {
            push_wrapped_panic(state, p);
            ffi::lua_error(state)
        }
    }
}

// In the context of a lua callback, this will call the given function and if the given function
// returns an error, *or if the given function panics*, this will result in a call to lua_error (a
// longjmp).  The error or panic is wrapped in such a way that when calling handle_error back on
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
// Error::CallbackError with a traceback.  If it is a lua error, creates a new string error with a
// printed traceback, and if it is a WrappedPanic, does not modify it.
pub unsafe extern "C" fn error_traceback(state: *mut ffi::lua_State) -> c_int {
    if let Some(error) = pop_wrapped_error(state) {
        ffi::luaL_traceback(state, state, ptr::null(), 0);
        let traceback = CStr::from_ptr(ffi::lua_tolstring(state, -1, ptr::null_mut()))
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
        let s = ffi::lua_tolstring(state, 1, ptr::null_mut());
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

// ffi::lua_pcall with a message handler that gives a nice traceback.  If the
// caught error is actually a Error, will simply pass the error along.  Does
// not call checkstack, and uses 2 extra stack spaces.
pub unsafe fn pcall_with_traceback(
    state: *mut ffi::lua_State,
    nargs: c_int,
    nresults: c_int,
) -> c_int {
    let msgh_position = ffi::lua_gettop(state) - nargs;
    ffi::lua_pushcfunction(state, error_traceback);
    ffi::lua_insert(state, msgh_position);
    let ret = ffi::lua_pcall(state, nargs, nresults, msgh_position);
    ffi::lua_remove(state, msgh_position);
    ret
}

pub unsafe fn resume_with_traceback(
    state: *mut ffi::lua_State,
    from: *mut ffi::lua_State,
    nargs: c_int,
) -> c_int {
    let res = ffi::lua_resume(state, from, nargs);
    if res != ffi::LUA_OK && res != ffi::LUA_YIELD {
        error_traceback(state);
    }
    res
}

// A variant of pcall that does not allow lua to catch panic errors from callback_error
pub unsafe extern "C" fn safe_pcall(state: *mut ffi::lua_State) -> c_int {
    let top = ffi::lua_gettop(state);
    if top == 0 {
        push_string(state, "not enough arguments to pcall");
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
        push_string(state, "not enough arguments to xpcall");
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

// Safely call setmetatable, if a __gc function is given, will wrap it in pcall, and panic on error.
pub unsafe extern "C" fn safe_setmetatable(state: *mut ffi::lua_State) -> c_int {
    if ffi::lua_gettop(state) < 2 {
        push_string(state, "not enough arguments to setmetatable");
        ffi::lua_error(state);
    }

    // Wrapping the __gc method in setmetatable ONLY works because Lua 5.3 only honors the __gc
    // method when it exists upon calling setmetatable, and ignores it if it is set later.
    push_string(state, "__gc");
    if ffi::lua_istable(state, -2) == 1 && ffi::lua_rawget(state, -2) == ffi::LUA_TFUNCTION {
        unsafe extern "C" fn safe_gc(state: *mut ffi::lua_State) -> c_int {
            ffi::lua_pushvalue(state, ffi::lua_upvalueindex(1));
            ffi::lua_insert(state, 1);
            if ffi::lua_pcall(state, 1, 0, 0) != ffi::LUA_OK {
                // If a user supplied __gc metamethod causes an error, we must always abort.  We may
                // be inside a protected context due to being in a callback, but inside an
                // unprotected ffi call that can cause memory errors, so may be at risk of
                // longjmping over arbitrary rust.
                eprintln!("Lua error during __gc, aborting!");
                process::abort()
            } else {
                ffi::lua_gettop(state)
            }
        }

        ffi::lua_pushcclosure(state, safe_gc, 1);
        push_string(state, "__gc");
        ffi::lua_insert(state, -2);
        ffi::lua_rawset(state, -3);
    } else {
        ffi::lua_pop(state, 1);
    }
    ffi::lua_setmetatable(state, -2);
    1
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

    push_userdata(state, WrappedError(err));

    get_error_metatable(state);
    ffi::lua_setmetatable(state, -2);
}

// Pops a WrappedError off of the top of the stack, if it is a WrappedError.  If
// it is not a WrappedError, returns None and does not pop anything.
pub unsafe fn pop_wrapped_error(state: *mut ffi::lua_State) -> Option<Error> {
    if ffi::lua_gettop(state) == 0 || !is_wrapped_error(state, -1) {
        None
    } else {
        let err = &*get_userdata::<WrappedError>(state, -1);
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

    push_userdata(state, WrappedPanic(Some(panic)));

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
    unsafe extern "C" fn error_tostring(state: *mut ffi::lua_State) -> c_int {
        callback_error(state, || {
            if is_wrapped_error(state, -1) {
                let error = get_userdata::<WrappedError>(state, -1);
                push_string(state, &(*error).0.to_string());
                ffi::lua_remove(state, -2);

                Ok(1)
            } else {
                panic!("internal error: userdata mismatch in Error metamethod");
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

        ffi::lua_newtable(state);
        ffi::lua_pushlightuserdata(
            state,
            &ERROR_METATABLE_REGISTRY_KEY as *const u8 as *mut c_void,
        );
        ffi::lua_pushvalue(state, -2);

        push_string(state, "__gc");
        ffi::lua_pushcfunction(state, userdata_destructor::<WrappedError>);
        ffi::lua_settable(state, -3);

        push_string(state, "__tostring");
        ffi::lua_pushcfunction(state, error_tostring);
        ffi::lua_settable(state, -3);

        push_string(state, "__metatable");
        ffi::lua_pushboolean(state, 0);
        ffi::lua_settable(state, -3);

        ffi::lua_settable(state, ffi::LUA_REGISTRYINDEX);
    }

    ffi::LUA_TTABLE
}

unsafe fn get_panic_metatable(state: *mut ffi::lua_State) -> c_int {
    ffi::lua_pushlightuserdata(
        state,
        &PANIC_METATABLE_REGISTRY_KEY as *const u8 as *mut c_void,
    );
    let t = ffi::lua_gettable(state, ffi::LUA_REGISTRYINDEX);

    if t != ffi::LUA_TTABLE {
        ffi::lua_pop(state, 1);

        ffi::luaL_checkstack(state, 8, ptr::null());

        ffi::lua_newtable(state);
        ffi::lua_pushlightuserdata(
            state,
            &PANIC_METATABLE_REGISTRY_KEY as *const u8 as *mut c_void,
        );
        ffi::lua_pushvalue(state, -2);

        push_string(state, "__gc");
        ffi::lua_pushcfunction(state, userdata_destructor::<WrappedPanic>);
        ffi::lua_settable(state, -3);

        push_string(state, "__metatable");
        ffi::lua_pushboolean(state, 0);
        ffi::lua_settable(state, -3);

        ffi::lua_settable(state, ffi::LUA_REGISTRYINDEX);
    }

    ffi::LUA_TTABLE
}

static ERROR_METATABLE_REGISTRY_KEY: u8 = 0;
static PANIC_METATABLE_REGISTRY_KEY: u8 = 0;
