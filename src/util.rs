use std::{mem, ptr};
use std::sync::Arc;
use std::ffi::CStr;
use std::any::Any;
use std::os::raw::{c_char, c_int, c_void};
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};

use ffi;
use error::{Error, Result};

// Checks that Lua has enough free stack space for future stack operations.  On failure, this will
// panic.
pub unsafe fn check_stack(state: *mut ffi::lua_State, amount: c_int) {
    rlua_assert!(
        ffi::lua_checkstack(state, amount) != 0,
        "out of stack space"
    );
}

// Similar to `check_stack`, but returns `Error::StackError` on failure.  Useful for user controlled
// sizes, which should not cause a panic.
pub unsafe fn check_stack_err(state: *mut ffi::lua_State, amount: c_int) -> Result<()> {
    if ffi::lua_checkstack(state, amount) == 0 {
        Err(Error::StackError)
    } else {
        Ok(())
    }
}

// Run an operation on a lua_State and check that the stack change is what is expected.  If the
// stack change does not match, resets the stack and panics.  If the given operation panics, tries
// to restore the stack to its previous state before resuming the panic.
pub unsafe fn stack_guard<F, R>(state: *mut ffi::lua_State, change: c_int, op: F) -> R
where
    F: FnOnce() -> R,
{
    let begin = ffi::lua_gettop(state);
    let expected = begin + change;
    rlua_assert!(expected >= 0, "too many stack values would be popped");

    let res = match catch_unwind(AssertUnwindSafe(op)) {
        Ok(r) => r,
        Err(p) => {
            let top = ffi::lua_gettop(state);
            if top > begin {
                ffi::lua_settop(state, begin);
            }
            resume_unwind(p);
        }
    };

    let top = ffi::lua_gettop(state);
    if top != expected {
        if top > begin {
            ffi::lua_settop(state, begin);
        }
        rlua_panic!("expected stack to be {}, got {}", expected, top);
    }

    res
}

// Run an operation on a lua_State and automatically clean up the stack before returning.  Takes the
// lua_State, the expected stack size change, and an operation to run.  If the operation results in
// success, then the stack is inspected to make sure the change in stack size matches the expected
// change and otherwise this is a logic error and will panic.  If the operation results in an error,
// the stack is shrunk to the value before the call.  If the operation results in an error and the
// stack is smaller than the value before the call, then this is unrecoverable and this will panic.
// If this function panics, it will clear the stack before panicking.
pub unsafe fn stack_err_guard<F, R>(state: *mut ffi::lua_State, change: c_int, op: F) -> Result<R>
where
    F: FnOnce() -> Result<R>,
{
    let begin = ffi::lua_gettop(state);
    let expected = begin + change;
    rlua_assert!(expected >= 0, "too many stack values would be popped");

    let res = match catch_unwind(AssertUnwindSafe(op)) {
        Ok(r) => r,
        Err(p) => {
            let top = ffi::lua_gettop(state);
            if top > begin {
                ffi::lua_settop(state, begin);
            }
            resume_unwind(p);
        }
    };

    let top = ffi::lua_gettop(state);
    if res.is_ok() {
        if top != expected {
            if top > begin {
                ffi::lua_settop(state, begin);
            }
            rlua_panic!("expected stack to be {}, got {}", expected, top);
        }
    } else {
        if top > expected {
            ffi::lua_settop(state, expected);
        } else if top < expected {
            rlua_panic!("{} too many stack values popped", top - begin - change);
        }
    }
    res
}

// Call a function that calls into the Lua API and may trigger a Lua error (longjmp) in a safe way.
// Wraps the inner function in a call to `lua_pcall`, so the inner function only has access to a
// limited lua stack.  `nargs` and `nresults` are similar to the parameters of `lua_pcall`, but the
// given function return type is not the return value count, instead the inner function return
// values are assumed to match the `nresults` param.  Internally uses 3 extra stack spaces, and does
// not call checkstack.  Provided function must *not* panic, and since it will generally be
// lonjmping, should not contain any values that implement Drop.
pub unsafe fn protect_lua_call<F, R>(
    state: *mut ffi::lua_State,
    nargs: c_int,
    nresults: c_int,
    f: F,
) -> Result<R>
where
    F: Fn(*mut ffi::lua_State) -> R,
    R: Copy,
{
    struct Params<F, R> {
        function: *const F,
        result: R,
        nresults: c_int,
    }

    unsafe extern "C" fn do_call<F, R>(state: *mut ffi::lua_State) -> c_int
    where
        F: Fn(*mut ffi::lua_State) -> R,
    {
        let params = ffi::lua_touserdata(state, -1) as *mut Params<F, R>;
        ffi::lua_pop(state, 1);

        (*params).result = (*(*params).function)(state);

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
        function: &f,
        result: mem::uninitialized(),
        nresults,
    };

    ffi::lua_pushlightuserdata(state, &mut params as *mut Params<F, R> as *mut c_void);
    let ret = ffi::lua_pcall(state, nargs + 1, nresults, stack_start + 1);
    ffi::lua_remove(state, stack_start + 1);

    if ret == ffi::LUA_OK {
        // LUA_OK is only returned when the do_call function has completed successfully, so
        // params.result is definitely initialized.
        Ok(params.result)
    } else {
        Err(pop_error(state, ret))
    }
}

// Pops an error off of the stack and returns it.  The specific behavior depends on the type of the
// error at the top of the stack:
//   1) If the error is actually a WrappedPanic, this will continue the panic.
//   2) If the error on the top of the stack is actually a WrappedError, just returns it.
//   3) Otherwise, interprets the error as the appropriate lua error.
// Uses 2 stack spaces, does not call lua_checkstack.
pub unsafe fn pop_error(state: *mut ffi::lua_State, err_code: c_int) -> Error {
    rlua_assert!(
        err_code != ffi::LUA_OK && err_code != ffi::LUA_YIELD,
        "pop_error called with non-error return code"
    );

    if let Some(err) = pop_wrapped_error(state) {
        err
    } else if is_wrapped_panic(state, -1) {
        let panic = get_userdata::<WrappedPanic>(state, -1);
        if let Some(p) = (*panic).0.take() {
            resume_unwind(p);
        } else {
            rlua_panic!("panic was resumed twice")
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
                rlua_abort!("impossible Lua allocation error, aborting!")
            }
            ffi::LUA_ERRGCMM => Error::GarbageCollectorError(err_string),
            _ => rlua_panic!("unrecognized lua error code"),
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
    let ud = protect_lua_call(state, 0, 1, move |state| {
        ffi::lua_newuserdata(state, mem::size_of::<T>()) as *mut T
    })?;
    ptr::write(ud, t);
    Ok(())
}

pub unsafe fn get_userdata<T>(state: *mut ffi::lua_State, index: c_int) -> *mut T {
    let ud = ffi::lua_touserdata(state, index) as *mut T;
    rlua_assert!(!ud.is_null(), "userdata pointer is null");
    ud
}

// Pops the userdata off of the top of the stack and returns it to rust, invalidating the lua
// userdata.
pub unsafe fn take_userdata<T>(state: *mut ffi::lua_State) -> T {
    // We set the metatable of userdata on __gc to a special table with no __gc method and with
    // metamethods that trigger an error on access.  We do this so that it will not be double
    // dropped, and also so that it cannot be used or identified as any particular userdata type
    // after the first call to __gc.
    get_destructed_userdata_metatable(state);
    ffi::lua_setmetatable(state, -2);
    let ud = ffi::lua_touserdata(state, -1) as *mut T;
    rlua_assert!(!ud.is_null(), "userdata pointer is null");
    ffi::lua_pop(state, 1);
    ptr::read(ud)
}

pub unsafe extern "C" fn userdata_destructor<T>(state: *mut ffi::lua_State) -> c_int {
    callback_error(state, || {
        take_userdata::<T>(state);
        Ok(0)
    })
}

// In the context of a lua callback, this will call the given function and if the given function
// returns an error, *or if the given function panics*, this will result in a call to lua_error (a
// longjmp).  The error or panic is wrapped in such a way that when calling pop_error back on
// the rust side, it will resume the panic.
pub unsafe fn callback_error<R, F>(state: *mut ffi::lua_State, f: F) -> R
where
    F: FnOnce() -> Result<R>,
{
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(r)) => r,
        Ok(Err(err)) => {
            ffi::lua_settop(state, 0);
            ffi::luaL_checkstack(state, 2, ptr::null());
            push_wrapped_error(state, err);
            ffi::lua_error(state)
        }
        Err(p) => {
            ffi::lua_settop(state, 0);
            if ffi::lua_checkstack(state, 2) == 0 {
                rlua_abort!("not enough stack space to propagate panic");
            }
            push_wrapped_panic(state, p);
            ffi::lua_error(state)
        }
    }
}

// Takes an error at the top of the stack, and if it is a WrappedError, converts it to an
// Error::CallbackError with a traceback, if it is some lua type, prints the error along with a
// traceback, and if it is a WrappedPanic, does not modify it.
pub unsafe extern "C" fn error_traceback(state: *mut ffi::lua_State) -> c_int {
    // I believe luaL_traceback requires this much free stack to not error.
    const LUA_TRACEBACK_STACK: c_int = 11;

    if ffi::lua_checkstack(state, 2) == 0 {
        // If we don't have enough stack space to even check the error type, do nothing
    } else if is_wrapped_error(state, 1) {
        let traceback = if ffi::lua_checkstack(state, LUA_TRACEBACK_STACK) != 0 {
            gc_guard(state, || {
                ffi::luaL_traceback(state, state, ptr::null(), 0);
            });
            let traceback = CStr::from_ptr(ffi::lua_tostring(state, -1))
                .to_string_lossy()
                .into_owned();
            ffi::lua_pop(state, 1);
            traceback
        } else {
            "not enough stack space for traceback".to_owned()
        };

        let error = pop_wrapped_error(state).unwrap();
        push_wrapped_error(
            state,
            Error::CallbackError {
                traceback,
                cause: Arc::new(error),
            },
        );
    } else if !is_wrapped_panic(state, 1) {
        if ffi::lua_checkstack(state, LUA_TRACEBACK_STACK) != 0 {
            gc_guard(state, || {
                let s = ffi::lua_tostring(state, 1);
                let s = if s.is_null() {
                    cstr!("<unprintable lua error>")
                } else {
                    s
                };
                ffi::luaL_traceback(state, state, s, 0);
                ffi::lua_remove(state, -2);
            });
        }
    }
    1
}

// A variant of pcall that does not allow lua to catch panic errors from callback_error
pub unsafe extern "C" fn safe_pcall(state: *mut ffi::lua_State) -> c_int {
    ffi::luaL_checkstack(state, 2, ptr::null());

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
        ffi::luaL_checkstack(state, 2, ptr::null());

        if is_wrapped_panic(state, -1) {
            1
        } else {
            ffi::lua_pushvalue(state, ffi::lua_upvalueindex(1));
            ffi::lua_insert(state, 1);
            ffi::lua_call(state, ffi::lua_gettop(state) - 1, ffi::LUA_MULTRET);
            ffi::lua_gettop(state)
        }
    }

    ffi::luaL_checkstack(state, 2, ptr::null());

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

// Does not call lua_checkstack, uses 1 stack space.
pub unsafe fn main_state(state: *mut ffi::lua_State) -> *mut ffi::lua_State {
    ffi::lua_rawgeti(state, ffi::LUA_REGISTRYINDEX, ffi::LUA_RIDX_MAINTHREAD);
    let main_state = ffi::lua_tothread(state, -1);
    ffi::lua_pop(state, 1);
    main_state
}

// Pushes a WrappedError::Error to the top of the stack.  Uses two stack spaces and does not call
// lua_checkstack.
pub unsafe fn push_wrapped_error(state: *mut ffi::lua_State, err: Error) {
    gc_guard(state, || {
        let ud = ffi::lua_newuserdata(state, mem::size_of::<WrappedError>()) as *mut WrappedError;
        ptr::write(ud, WrappedError(err))
    });

    get_error_metatable(state);
    ffi::lua_setmetatable(state, -2);
}

// Pops a WrappedError off of the top of the stack, if it is a WrappedError.  If it is not a
// WrappedError, returns None and does not pop anything.  Uses 2 stack spaces and does not call
// lua_checkstack.
pub unsafe fn pop_wrapped_error(state: *mut ffi::lua_State) -> Option<Error> {
    if !is_wrapped_error(state, -1) {
        None
    } else {
        let err = &*get_userdata::<WrappedError>(state, -1);
        // We are assuming here that Error::clone() cannot panic.
        let err = err.0.clone();
        ffi::lua_pop(state, 1);
        Some(err)
    }
}

// Runs the given function with the Lua garbage collector disabled.  `rlua` assumes that all
// allocation failures are aborts, so when the garbage collector is disabled, 'm' functions that can
// cause either an allocation error or a a `__gc` metamethod error are prevented from causing errors
// at all.  The given function should never panic or longjmp, because this could inadverntently
// disable the gc.  This is useful when error handling must allocate, and `__gc` errors at that time
// would shadow more important errors, or be extremely difficult to handle safely.
pub unsafe fn gc_guard<R, F: FnOnce() -> R>(state: *mut ffi::lua_State, f: F) -> R {
    if ffi::lua_gc(state, ffi::LUA_GCISRUNNING, 0) != 0 {
        ffi::lua_gc(state, ffi::LUA_GCSTOP, 0);
        let r = f();
        ffi::lua_gc(state, ffi::LUA_GCRESTART, 0);
        r
    } else {
        f()
    }
}

// Initialize the error, panic, and destructed userdata metatables.
pub unsafe fn init_error_metatables(state: *mut ffi::lua_State) {
    check_stack(state, 8);

    // Create error metatable

    unsafe extern "C" fn error_tostring(state: *mut ffi::lua_State) -> c_int {
        ffi::luaL_checkstack(state, 2, ptr::null());

        callback_error(state, || {
            if is_wrapped_error(state, -1) {
                let error = get_userdata::<WrappedError>(state, -1);
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
    ffi::lua_newtable(state);

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

    // Create panic metatable

    ffi::lua_pushlightuserdata(
        state,
        &PANIC_METATABLE_REGISTRY_KEY as *const u8 as *mut c_void,
    );
    ffi::lua_newtable(state);

    ffi::lua_pushstring(state, cstr!("__gc"));
    ffi::lua_pushcfunction(state, userdata_destructor::<WrappedPanic>);
    ffi::lua_rawset(state, -3);

    ffi::lua_pushstring(state, cstr!("__metatable"));
    ffi::lua_pushboolean(state, 0);
    ffi::lua_rawset(state, -3);

    ffi::lua_rawset(state, ffi::LUA_REGISTRYINDEX);

    // Create destructed userdata metatable

    unsafe extern "C" fn destructed_error(state: *mut ffi::lua_State) -> c_int {
        ffi::luaL_checkstack(state, 2, ptr::null());
        push_wrapped_error(state, Error::CallbackDestructed);
        ffi::lua_error(state)
    }

    ffi::lua_pushlightuserdata(
        state,
        &DESTRUCTED_USERDATA_METATABLE as *const u8 as *mut c_void,
    );
    ffi::lua_newtable(state);

    for &method in &[
        cstr!("__add"),
        cstr!("__sub"),
        cstr!("__mul"),
        cstr!("__div"),
        cstr!("__mod"),
        cstr!("__pow"),
        cstr!("__unm"),
        cstr!("__idiv"),
        cstr!("__band"),
        cstr!("__bor"),
        cstr!("__bxor"),
        cstr!("__bnot"),
        cstr!("__shl"),
        cstr!("__shr"),
        cstr!("__concat"),
        cstr!("__len"),
        cstr!("__eq"),
        cstr!("__lt"),
        cstr!("__le"),
        cstr!("__index"),
        cstr!("__newindex"),
        cstr!("__call"),
        cstr!("__tostring"),
        cstr!("__pairs"),
        cstr!("__ipairs"),
    ] {
        ffi::lua_pushstring(state, method);
        ffi::lua_pushcfunction(state, destructed_error);
        ffi::lua_rawset(state, -3);
    }

    ffi::lua_rawset(state, ffi::LUA_REGISTRYINDEX);
}

struct WrappedError(pub Error);
struct WrappedPanic(pub Option<Box<Any + Send>>);

// Pushes a WrappedError::Panic to the top of the stack.  Uses two stack spaces and does not call
// lua_checkstack.
unsafe fn push_wrapped_panic(state: *mut ffi::lua_State, panic: Box<Any + Send>) {
    gc_guard(state, || {
        let ud = ffi::lua_newuserdata(state, mem::size_of::<WrappedPanic>()) as *mut WrappedPanic;
        ptr::write(ud, WrappedPanic(Some(panic)))
    });

    get_panic_metatable(state);
    ffi::lua_setmetatable(state, -2);
}

// Checks if the value at the given index is a WrappedError, uses 2 stack spaces and does not call
// lua_checkstack.
unsafe fn is_wrapped_error(state: *mut ffi::lua_State, index: c_int) -> bool {
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

// Checks if the value at the given index is a WrappedPanic.  Uses 2 stack spaces and does not call
// lua_checkstack.
unsafe fn is_wrapped_panic(state: *mut ffi::lua_State, index: c_int) -> bool {
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

unsafe fn get_error_metatable(state: *mut ffi::lua_State) {
    ffi::lua_pushlightuserdata(
        state,
        &ERROR_METATABLE_REGISTRY_KEY as *const u8 as *mut c_void,
    );
    ffi::lua_rawget(state, ffi::LUA_REGISTRYINDEX);
}

unsafe fn get_panic_metatable(state: *mut ffi::lua_State) {
    ffi::lua_pushlightuserdata(
        state,
        &PANIC_METATABLE_REGISTRY_KEY as *const u8 as *mut c_void,
    );
    ffi::lua_rawget(state, ffi::LUA_REGISTRYINDEX);
}

unsafe fn get_destructed_userdata_metatable(state: *mut ffi::lua_State) {
    ffi::lua_pushlightuserdata(
        state,
        &DESTRUCTED_USERDATA_METATABLE as *const u8 as *mut c_void,
    );
    ffi::lua_rawget(state, ffi::LUA_REGISTRYINDEX);
}

static ERROR_METATABLE_REGISTRY_KEY: u8 = 0;
static PANIC_METATABLE_REGISTRY_KEY: u8 = 0;
static DESTRUCTED_USERDATA_METATABLE: u8 = 0;
