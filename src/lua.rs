use std::any::TypeId;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::os::raw::{c_int, c_void};
use std::sync::{Arc, Mutex};
use std::{ptr, str};

use libc;

use context::Context;
use ffi;
use markers::NoRefUnwindSafe;
use types::Callback;
use util::{
    assert_stack, init_error_metatables, protect_lua_closure, safe_pcall, safe_xpcall,
    userdata_destructor,
};

/// Top level Lua struct which holds the Lua state itself.
pub struct Lua {
    main_state: *mut ffi::lua_State,
    _no_ref_unwind_safe: NoRefUnwindSafe,
}

unsafe impl Send for Lua {}

impl Drop for Lua {
    fn drop(&mut self) {
        unsafe {
            let extra = extra_data(self.main_state);
            rlua_debug_assert!(
                ffi::lua_gettop((*extra).ref_thread) == (*extra).ref_stack_max
                    && (*extra).ref_stack_max as usize == (*extra).ref_free.len(),
                "reference leak detected"
            );
            *rlua_expect!((*extra).registry_unref_list.lock(), "unref list poisoned") = None;
            Box::from_raw(extra);

            ffi::lua_close(self.main_state);
        }
    }
}

impl Lua {
    /// Creates a new Lua state and loads standard library without the `debug` library.
    pub fn new() -> Lua {
        unsafe { create_lua(false) }
    }

    /// Creates a new Lua state and loads the standard library including the `debug` library.
    ///
    /// The debug library is very unsound, loading it and using it breaks all the guarantees of
    /// rlua.
    pub unsafe fn new_with_debug() -> Lua {
        create_lua(true)
    }

    /// The main entry point of the rlua API.
    ///
    /// rlua has reference types like `String` and `Table` which reference shared data in the Lua
    /// state.  These are special reference counted types that contain pointers to the main Lua
    /// state via the `Context` type, and there is a `'lua` lifetime associated with these.
    ///
    /// This `'lua` lifetime is somewhat special.  It is what is sometimes called a "generative"
    /// lifetime or a "branding" lifetime, which is unique for each call to `Lua::context` and
    /// is invariant.
    ///
    /// The reason this entry point must be a callback is so that this unique lifetime can be
    /// generated as part of the callback's parameters.  Even though this callback API is somewhat
    /// inconvenient, it has several advantages:
    ///
    /// - Inside calls to `Lua::context`, we know that all instances of the 'lua lifetime are the
    ///   same unique lifetime.  Thus, it is impossible for the user to accidentally mix handle
    ///   types between different instances of `Lua`.
    /// - Because we know at compile time that handles cannot be mixed from different instances of
    ///   `Lua`, we do not need to do runtime checks to make sure that handles are from the same
    ///   state.
    /// - Handle types cannot escape the context call and the `'lua` context lifetime is in general
    ///   very limited, preventing it from being stored in unexpected places.  This is a benefit as
    ///   it helps ensure the soundness of the API.
    ///
    /// It is not possible to return types with this `'lua` context lifetime from the given
    /// callback, or store them long-term in any way.  There is an escape hatch here, though: if you
    /// need to keep references to internal Lua values long-term, you can use the Lua registry via
    /// `Lua::set_named_registry_value` and `Lua::create_registry_value`.
    pub fn context<F, R>(&self, f: F) -> R
    where
        F: FnOnce(Context) -> R,
    {
        f(unsafe { Context::new(self.main_state) })
    }
}

// Data associated with the main lua_State via lua_getextraspace.
pub(crate) struct ExtraData {
    pub registered_userdata: HashMap<TypeId, c_int>,
    pub registry_unref_list: Arc<Mutex<Option<Vec<c_int>>>>,

    pub ref_thread: *mut ffi::lua_State,
    pub ref_stack_size: c_int,
    pub ref_stack_max: c_int,
    pub ref_free: Vec<c_int>,
}

pub(crate) unsafe fn extra_data(state: *mut ffi::lua_State) -> *mut ExtraData {
    *(ffi::lua_getextraspace(state) as *mut *mut ExtraData)
}

unsafe fn create_lua(load_debug: bool) -> Lua {
    unsafe extern "C" fn allocator(
        _: *mut c_void,
        ptr: *mut c_void,
        _: usize,
        nsize: usize,
    ) -> *mut c_void {
        if nsize == 0 {
            libc::free(ptr as *mut libc::c_void);
            ptr::null_mut()
        } else {
            let p = libc::realloc(ptr as *mut libc::c_void, nsize);
            if p.is_null() {
                // We require that OOM results in an abort, and that the lua allocator function
                // never errors.  Since this is what rust itself normally does on OOM, this is
                // not really a huge loss.  Importantly, this allows us to turn off the gc, and
                // then know that calling Lua API functions marked as 'm' will not result in a
                // 'longjmp' error while the gc is off.
                abort!("out of memory in rlua::Lua allocation, aborting!");
            } else {
                p as *mut c_void
            }
        }
    }

    let state = ffi::lua_newstate(allocator, ptr::null_mut());

    let ref_thread = rlua_expect!(
        protect_lua_closure(state, 0, 0, |state| {
            // Do not open the debug library, it can be used to cause unsafety.
            ffi::luaL_requiref(state, cstr!("_G"), ffi::luaopen_base, 1);
            ffi::luaL_requiref(state, cstr!("coroutine"), ffi::luaopen_coroutine, 1);
            ffi::luaL_requiref(state, cstr!("table"), ffi::luaopen_table, 1);
            ffi::luaL_requiref(state, cstr!("io"), ffi::luaopen_io, 1);
            ffi::luaL_requiref(state, cstr!("os"), ffi::luaopen_os, 1);
            ffi::luaL_requiref(state, cstr!("string"), ffi::luaopen_string, 1);
            ffi::luaL_requiref(state, cstr!("utf8"), ffi::luaopen_utf8, 1);
            ffi::luaL_requiref(state, cstr!("math"), ffi::luaopen_math, 1);
            ffi::luaL_requiref(state, cstr!("package"), ffi::luaopen_package, 1);
            ffi::lua_pop(state, 9);

            init_error_metatables(state);

            if load_debug {
                ffi::luaL_requiref(state, cstr!("debug"), ffi::luaopen_debug, 1);
                ffi::lua_pop(state, 1);
            }

            // Create the function metatable

            ffi::lua_pushlightuserdata(
                state,
                &FUNCTION_METATABLE_REGISTRY_KEY as *const u8 as *mut c_void,
            );

            ffi::lua_newtable(state);

            ffi::lua_pushstring(state, cstr!("__gc"));
            ffi::lua_pushcfunction(state, userdata_destructor::<Callback>);
            ffi::lua_rawset(state, -3);

            ffi::lua_pushstring(state, cstr!("__metatable"));
            ffi::lua_pushboolean(state, 0);
            ffi::lua_rawset(state, -3);

            ffi::lua_rawset(state, ffi::LUA_REGISTRYINDEX);

            // Override pcall and xpcall with versions that cannot be used to catch rust panics.

            ffi::lua_rawgeti(state, ffi::LUA_REGISTRYINDEX, ffi::LUA_RIDX_GLOBALS);

            ffi::lua_pushstring(state, cstr!("pcall"));
            ffi::lua_pushcfunction(state, safe_pcall);
            ffi::lua_rawset(state, -3);

            ffi::lua_pushstring(state, cstr!("xpcall"));
            ffi::lua_pushcfunction(state, safe_xpcall);
            ffi::lua_rawset(state, -3);

            ffi::lua_pop(state, 1);

            // Create ref stack thread and place it in the registry to prevent it from being garbage
            // collected.

            let ref_thread = ffi::lua_newthread(state);
            ffi::luaL_ref(state, ffi::LUA_REGISTRYINDEX);

            ref_thread
        }),
        "Error during Lua construction",
    );

    rlua_debug_assert!(ffi::lua_gettop(state) == 0, "stack leak during creation");
    assert_stack(state, ffi::LUA_MINSTACK);

    // Create ExtraData, and place it in the lua_State "extra space"

    let extra = Box::into_raw(Box::new(ExtraData {
        registered_userdata: HashMap::new(),
        registry_unref_list: Arc::new(Mutex::new(Some(Vec::new()))),
        ref_thread,
        // We need 1 extra stack space to move values in and out of the ref stack.
        ref_stack_size: ffi::LUA_MINSTACK - 1,
        ref_stack_max: 0,
        ref_free: Vec::new(),
    }));
    *(ffi::lua_getextraspace(state) as *mut *mut ExtraData) = extra;

    Lua {
        main_state: state,
        _no_ref_unwind_safe: PhantomData,
    }
}

pub(crate) static FUNCTION_METATABLE_REGISTRY_KEY: u8 = 0;
