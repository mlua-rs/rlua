use std::mem;
use std::ptr;
use std::process;
use std::sync::Arc;
use std::ffi::CStr;
use std::any::Any;
use std::os::raw::{c_char, c_int, c_void};
use std::panic::{catch_unwind, resume_unwind, UnwindSafe};

use ffi;
use error::{LuaResult, LuaError};

macro_rules! cstr {
  ($s:expr) => (
    concat!($s, "\0") as *const str as *const [c_char] as *const c_char
  );
}

pub unsafe fn check_stack(state: *mut ffi::lua_State, amount: c_int) -> LuaResult<()> {
    if ffi::lua_checkstack(state, amount) == 0 {
        Err(LuaError::StackOverflow)
    } else {
        Ok(())
    }
}

// Run an operation on a lua_State and automatically clean up the stack before returning.  Takes
// the lua_State, the expected stack size change, and an operation to run.  If the operation
// results in success, then the stack is inspected to make sure the change in stack size matches
// the expected change and otherwise this is a logic error and will panic.  If the operation
// results in an error, the stack is shrunk to the value before the call.  If the operation
// results in an error and the stack is smaller than the value before the call, then this is
// unrecoverable and this will panic.
pub unsafe fn stack_guard<F, R>(state: *mut ffi::lua_State, change: c_int, op: F) -> LuaResult<R>
where
    F: FnOnce() -> LuaResult<R>,
{
    let expected = ffi::lua_gettop(state) + change;
    assert!(
        expected >= 0,
        "lua stack error, too many values would be popped"
    );
    let res = op();
    let top = ffi::lua_gettop(state);
    if res.is_ok() {
        assert_eq!(
            ffi::lua_gettop(state),
            expected,
            "lua stack error, expected stack to be {}, got {}",
            expected,
            top
        );
    } else {
        assert!(
            top >= expected,
            "lua stack error, {} too many values popped",
            top - expected
        );
        if top > expected {
            ffi::lua_settop(state, expected);
        }
    }
    res
}

// Call the given rust function in a protected lua context, similar to pcall.
// The stack given to the protected function is a separate protected stack. This
// catches all calls to lua_error, but ffi functions that can call lua_error are
// still longjmps, and have all the same dangers as longjmps, so extreme care
// must still be taken in code that uses this function.  Does not call
// lua_checkstack, and uses 2 extra stack spaces.
pub unsafe fn error_guard<F, R>(
    state: *mut ffi::lua_State,
    nargs: c_int,
    nresults: c_int,
    func: F,
) -> LuaResult<R>
where
    F: FnOnce(*mut ffi::lua_State) -> LuaResult<R> + UnwindSafe,
{
    unsafe extern "C" fn call_impl<F>(state: *mut ffi::lua_State) -> c_int
    where
        F: FnOnce(*mut ffi::lua_State) -> c_int,
    {
        let func = ffi::lua_touserdata(state, -1) as *mut F;
        let func = mem::replace(&mut *func, mem::uninitialized());
        ffi::lua_pop(state, 1);
        func(state)
    }

    unsafe fn cpcall<F>(
        state: *mut ffi::lua_State,
        nargs: c_int,
        nresults: c_int,
        mut func: F,
    ) -> LuaResult<()>
    where
        F: FnOnce(*mut ffi::lua_State) -> c_int,
    {
        ffi::lua_pushcfunction(state, call_impl::<F>);
        ffi::lua_insert(state, -(nargs + 1));
        ffi::lua_pushlightuserdata(state, &mut func as *mut F as *mut c_void);
        mem::forget(func);
        handle_error(state, pcall_with_traceback(state, nargs + 1, nresults))
    }

    let mut res = None;
    cpcall(state, nargs, nresults, |state| {
        res = Some(callback_error(state, || func(state)));
        ffi::lua_gettop(state)
    })?;
    Ok(res.unwrap())
}

// If the return code indicates an error, pops the error off of the stack and
// returns Err. If the error is actually a WrappedPaic, clears the current lua
// stack continues the panic.  If the error on the top of the stack is actually
// a WrappedError, just returns it.  Otherwise, interprets the error as the
// appropriate lua error.
pub unsafe fn handle_error(state: *mut ffi::lua_State, err: c_int) -> LuaResult<()> {
    if err == ffi::LUA_OK || err == ffi::LUA_YIELD {
        Ok(())
    } else {
        if let Some(err) = pop_wrapped_error(state) {
            Err(err)

        } else if is_wrapped_panic(state, -1) {
            let userdata = ffi::lua_touserdata(state, -1);
            let panic = &mut *(userdata as *mut WrappedPanic);
            resume_unwind(panic.0.take().expect(
                "internal error: panic was resumed twice",
            ))

        } else {
            let err_string =
                if let Some(s) = ffi::lua_tolstring(state, -1, ptr::null_mut()).as_ref() {
                    CStr::from_ptr(s)
                        .to_str()
                        .unwrap_or_else(|_| "<unprintable error>")
                        .to_owned()
                } else {
                    "<unprintable error>".to_owned()
                };
            ffi::lua_pop(state, 1);

            Err(match err {
                ffi::LUA_ERRRUN => LuaError::RuntimeError(err_string),
                ffi::LUA_ERRSYNTAX => {
                    // This seems terrible, but as far as I can tell, this is exactly what the stock lua
                    // repl does.
                    if err_string.ends_with("<eof>") {
                        LuaError::IncompleteStatement(err_string)
                    } else {
                        LuaError::SyntaxError(err_string)
                    }
                }
                ffi::LUA_ERRERR => LuaError::ErrorError(err_string),
                ffi::LUA_ERRMEM => {
                    // This is not impossible to hit, but this library is not set up
                    // to handle this properly.  Lua does a longjmp on out of memory
                    // (like all lua errors), but it can do this from a huge number
                    // of lua functions, and it is extremely difficult to set up the
                    // pcall protection for every lua function that might allocate.
                    // If lua does this in an unprotected context, it will abort
                    // anyway, so the best we can do right now is guarantee an abort
                    // even in a protected context.
                    println!("Lua memory error, aborting!");
                    process::abort()
                }
                ffi::LUA_ERRGCMM => {
                    // This should be impossible, or at least is indicative of an
                    // internal bug.  Similarly to LUA_ERRMEM, this could indicate a
                    // longjmp out of rust code, so we just abort.
                    println!("Lua error during __gc, aborting!");
                    process::abort()
                }
                _ => panic!("unrecognized lua error code"),
            })
        }
    }
}

pub unsafe fn push_string(state: *mut ffi::lua_State, s: &str) {
    ffi::lua_pushlstring(state, s.as_ptr() as *const c_char, s.len());
}

pub unsafe extern "C" fn destructor<T>(state: *mut ffi::lua_State) -> c_int {
    match catch_unwind(|| {
        let obj = &mut *(ffi::lua_touserdata(state, 1) as *mut T);
        mem::replace(obj, mem::uninitialized());
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
    F: FnOnce() -> LuaResult<R> + UnwindSafe,
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

// ffi::lua_pcall with a message handler that gives a nice traceback.  If the
// caught error is actually a LuaError, will simply pass the error along.  Does
// not call checkstack, and uses 2 extra stack spaces.
pub unsafe fn pcall_with_traceback(
    state: *mut ffi::lua_State,
    nargs: c_int,
    nresults: c_int,
) -> c_int {
    unsafe extern "C" fn message_handler(state: *mut ffi::lua_State) -> c_int {
        if let Some(error) = pop_wrapped_error(state) {
            ffi::luaL_traceback(state, state, ptr::null(), 0);
            let traceback = CStr::from_ptr(ffi::lua_tolstring(state, -1, ptr::null_mut()))
                .to_str()
                .unwrap_or_else(|_| "<could not capture traceback>")
                .to_owned();
            push_wrapped_error(state, LuaError::CallbackError(traceback, Arc::new(error)));
        } else if !is_wrapped_panic(state, 1) {
            let s = ffi::lua_tolstring(state, 1, ptr::null_mut());
            if !s.is_null() {
                ffi::luaL_traceback(state, state, s, 0);
            } else {
                ffi::luaL_traceback(state, state, cstr!("<unprintable lua error>"), 0);
            }
        }
        1
    }

    let msgh_position = ffi::lua_gettop(state) - nargs;
    ffi::lua_pushcfunction(state, message_handler);
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
        if let Some(error) = pop_wrapped_error(state) {
            ffi::luaL_traceback(from, state, ptr::null(), 0);
            let traceback = CStr::from_ptr(ffi::lua_tolstring(from, -1, ptr::null_mut()))
                .to_str()
                .unwrap_or_else(|_| "<could not capture traceback>")
                .to_owned();
            push_wrapped_error(from, LuaError::CallbackError(traceback, Arc::new(error)));
        } else if !is_wrapped_panic(state, 1) {
            let s = ffi::lua_tolstring(state, 1, ptr::null_mut());
            if !s.is_null() {
                ffi::luaL_traceback(from, state, s, 0);
            } else {
                ffi::luaL_traceback(from, state, cstr!("<unprintable lua error>"), 0);
            }
        }
    }
    res
}

// A variant of pcall that does not allow lua to catch panic errors from callback_error
pub unsafe extern "C" fn safe_pcall(state: *mut ffi::lua_State) -> c_int {
    if ffi::lua_pcall(state, ffi::lua_gettop(state) - 1, ffi::LUA_MULTRET, 0) != ffi::LUA_OK {
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

    ffi::lua_pushvalue(state, 2);
    ffi::lua_pushcclosure(state, xpcall_msgh, 1);
    ffi::lua_copy(state, 1, 2);
    ffi::lua_insert(state, 1);

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
        ffi::lua_insert(state, 1);
        ffi::lua_gettop(state)
    }
}

/// Does not call checkstack, uses 1 stack space
pub unsafe fn main_state(state: *mut ffi::lua_State) -> *mut ffi::lua_State {
    ffi::lua_rawgeti(state, ffi::LUA_REGISTRYINDEX, ffi::LUA_RIDX_MAINTHREAD);
    let state = ffi::lua_tothread(state, -1);
    ffi::lua_pop(state, 1);
    state
}

pub struct WrappedError(pub LuaError);
pub struct WrappedPanic(pub Option<Box<Any + Send>>);

// Pushes a WrappedError::Error to the top of the stack
pub unsafe fn push_wrapped_error(state: *mut ffi::lua_State, err: LuaError) {
    unsafe extern "C" fn error_tostring(state: *mut ffi::lua_State) -> c_int {
        callback_error(state, || if is_wrapped_error(state, -1) {
            let userdata = ffi::lua_touserdata(state, -1);
            let error = &*(userdata as *const WrappedError);
            push_string(state, &error.0.to_string());
            ffi::lua_remove(state, -2);

            Ok(1)

        } else {
            Err(LuaError::FromLuaConversionError(
                "internal error: userdata mismatch in LuaError metamethod"
                    .to_owned(),
            ))
        })
    }

    ffi::luaL_checkstack(state, 2, ptr::null());

    let err_userdata = ffi::lua_newuserdata(state, mem::size_of::<WrappedError>()) as
        *mut WrappedError;

    ptr::write(err_userdata, WrappedError(err));

    get_error_metatable(state);
    if ffi::lua_isnil(state, -1) != 0 {
        ffi::lua_pop(state, 1);

        ffi::luaL_checkstack(state, 7, ptr::null());

        ffi::lua_newtable(state);
        ffi::lua_pushlightuserdata(
            state,
            &ERROR_METATABLE_REGISTRY_KEY as *const u8 as *mut c_void,
        );
        ffi::lua_pushvalue(state, -2);

        push_string(state, "__gc");
        ffi::lua_pushcfunction(state, destructor::<WrappedError>);
        ffi::lua_settable(state, -3);

        push_string(state, "__tostring");
        ffi::lua_pushcfunction(state, error_tostring);
        ffi::lua_settable(state, -3);

        push_string(state, "__metatable");
        ffi::lua_pushboolean(state, 0);
        ffi::lua_settable(state, -3);

        ffi::lua_settable(state, ffi::LUA_REGISTRYINDEX);
    }

    ffi::lua_setmetatable(state, -2);
}

// Pushes a WrappedError::Panic to the top of the stack
pub unsafe fn push_wrapped_panic(state: *mut ffi::lua_State, panic: Box<Any + Send>) {
    ffi::luaL_checkstack(state, 2, ptr::null());

    let panic_userdata = ffi::lua_newuserdata(state, mem::size_of::<WrappedPanic>()) as
        *mut WrappedPanic;

    ptr::write(panic_userdata, WrappedPanic(Some(panic)));

    get_panic_metatable(state);
    if ffi::lua_isnil(state, -1) != 0 {
        ffi::lua_pop(state, 1);

        ffi::luaL_checkstack(state, 7, ptr::null());

        ffi::lua_newtable(state);
        ffi::lua_pushlightuserdata(
            state,
            &PANIC_METATABLE_REGISTRY_KEY as *const u8 as *mut c_void,
        );
        ffi::lua_pushvalue(state, -2);

        push_string(state, "__gc");
        ffi::lua_pushcfunction(state, destructor::<WrappedPanic>);
        ffi::lua_settable(state, -3);

        push_string(state, "__metatable");
        ffi::lua_pushboolean(state, 0);
        ffi::lua_settable(state, -3);

        ffi::lua_settable(state, ffi::LUA_REGISTRYINDEX);
    }

    ffi::lua_setmetatable(state, -2);
}

// Pops a WrappedError off of the top of the stack, if it is a WrappedError.  If
// it is not a WrappedError, returns None and does not pop anything.
pub unsafe fn pop_wrapped_error(state: *mut ffi::lua_State) -> Option<LuaError> {
    if ffi::lua_gettop(state) == 0 || !is_wrapped_error(state, -1) {
        None
    } else {
        let userdata = ffi::lua_touserdata(state, -1);
        let err = &*(userdata as *const WrappedError);
        let err = err.0.clone();
        ffi::lua_pop(state, 1);
        Some(err)
    }
}

// Checks if the value at the given index is a WrappedError
pub unsafe fn is_wrapped_error(state: *mut ffi::lua_State, index: c_int) -> bool {
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
pub unsafe fn is_wrapped_panic(state: *mut ffi::lua_State, index: c_int) -> bool {
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
    return res;
}

pub unsafe fn get_error_metatable(state: *mut ffi::lua_State) -> c_int {
    ffi::lua_pushlightuserdata(
        state,
        &ERROR_METATABLE_REGISTRY_KEY as *const u8 as *mut c_void,
    );
    ffi::lua_gettable(state, ffi::LUA_REGISTRYINDEX)
}

pub unsafe fn get_panic_metatable(state: *mut ffi::lua_State) -> c_int {
    ffi::lua_pushlightuserdata(
        state,
        &PANIC_METATABLE_REGISTRY_KEY as *const u8 as *mut c_void,
    );
    ffi::lua_gettable(state, ffi::LUA_REGISTRYINDEX)
}

static ERROR_METATABLE_REGISTRY_KEY: u8 = 0;
static PANIC_METATABLE_REGISTRY_KEY: u8 = 0;
