use std::any::TypeId;
use std::cell::RefCell;
use std::collections::HashMap;
use std::marker::PhantomData;
#[cfg(rlua_lua54)]
use std::os::raw::c_uint;
use std::os::raw::{c_int, c_void};
use std::ptr;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use bitflags::bitflags;
use libc;

use crate::context::Context;
use crate::error::Result;
use crate::ffi;
use crate::hook::{hook_proc, Debug, HookTriggers};
use crate::markers::NoRefUnwindSafe;
use crate::types::Callback;
use crate::util::{
    assert_stack, dostring, init_error_registry, protect_lua_closure, push_globaltable, rawlen,
    requiref, safe_pcall, safe_xpcall, userdata_destructor,
};

bitflags! {
    /// Flags describing the set of lua modules to load.
    pub struct StdLib: u32 {
        const BASE = 0x1;
        const COROUTINE = 0x2;
        const TABLE = 0x4;
        const IO = 0x8;
        const OS = 0x10;
        const STRING = 0x20;
        const UTF8 = 0x40;
        const MATH = 0x80;
        const PACKAGE = 0x100;
        const DEBUG = 0x200;

        const ALL = StdLib::BASE.bits
            | StdLib::COROUTINE.bits
            | StdLib::TABLE.bits
            | StdLib::IO.bits
            | StdLib::OS.bits
            | StdLib::STRING.bits
            | StdLib::UTF8.bits
            | StdLib::MATH.bits
            | StdLib::PACKAGE.bits
            | StdLib::DEBUG.bits;

        const ALL_NO_DEBUG = StdLib::BASE.bits
            | StdLib::COROUTINE.bits
            | StdLib::TABLE.bits
            | StdLib::IO.bits
            | StdLib::OS.bits
            | StdLib::STRING.bits
            | StdLib::UTF8.bits
            | StdLib::MATH.bits
            | StdLib::PACKAGE.bits;
    }
}

bitflags! {
    /// Flags describing the set of lua modules to load.
    pub struct InitFlags: u32 {
        const PCALL_WRAPPERS = 0x1;
        const LOAD_WRAPPERS = 0x2;
        const REMOVE_LOADLIB = 0x4;

        const DEFAULT = InitFlags::PCALL_WRAPPERS.bits |
                        InitFlags::LOAD_WRAPPERS.bits |
                        InitFlags::REMOVE_LOADLIB.bits;
        const NONE = 0;
    }
}

#[cfg(rlua_lua54)]
// at 812, tests pass
// at 813, tests fail
// at 700, it should be somewhat safe
// TODO: move this somewhere nicer
// TODO: make this configurable?
const SAFE_CSTACK_SIZE: c_uint = 700;

/// Top level Lua struct which holds the Lua state itself.
#[derive(Debug)]
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
            ffi::lua_close(self.main_state);
            Box::from_raw(extra);
        }
    }
}

impl Lua {
    /// Creates a new Lua state and loads standard library without the `debug` library.
    pub fn new() -> Lua {
        unsafe { create_lua(StdLib::ALL_NO_DEBUG, InitFlags::DEFAULT) }
    }

    /// Creates a new Lua state and loads the standard library including the `debug` library.
    ///
    /// The debug library is very unsound, it can be used to break the safety guarantees of rlua.
    pub unsafe fn new_with_debug() -> Lua {
        create_lua(StdLib::ALL, InitFlags::DEFAULT)
    }

    /// Creates a new Lua state and loads a subset of the standard libraries.
    ///
    /// Use the [`StdLib`] flags to specifiy the libraries you want to load.
    ///
    /// Note that the `debug` library can't be loaded using this function as it can be used to break
    /// the safety guarantees of rlua.  If you really want to load it, use the sister function
    /// [`Lua::unsafe_new_with`].
    ///
    /// # Panics
    ///
    /// Panics if `lua_mod` contains `StdLib::DEBUG`
    pub fn new_with(lua_mod: StdLib) -> Lua {
        assert!(
            !lua_mod.contains(StdLib::DEBUG),
            "The lua debug module can't be loaded using `new_with`. Use `unsafe_new_with` instead."
        );

        unsafe { create_lua(lua_mod, InitFlags::DEFAULT) }
    }

    /// Creates a new Lua state and loads a subset of the standard libraries.
    ///
    /// Use the [`StdLib`] flags to specifiy the libraries you want to load.
    ///
    /// This function is unsafe because it can be used to load the `debug` library which can be used
    /// to break the safety guarantees provided by rlua.
    pub unsafe fn unsafe_new_with(lua_mod: StdLib) -> Lua {
        create_lua(lua_mod, InitFlags::DEFAULT)
    }

    /// Creates a new Lua state with a subset of the standard libraries and
    /// modified initialization.
    ///
    /// Use the [`StdLib`] flags to specifiy the libraries you want to load.
    /// Use the [`InitFlags`] to specify non-default Lua configuration.
    ///
    /// `unsafe_new_with_flags(mods, InitFlags::DEFAULT)` is equivalent to
    /// `unsafe_new_with(mods)`.
    ///
    /// This function is unsafe because it can be used to load the `debug` library which can be used
    /// to break the safety guarantees provided by rlua, or to disable some of the safety features
    /// which rlua provides by default.
    pub unsafe fn unsafe_new_with_flags(lua_mod: StdLib, init_flags: InitFlags) -> Lua {
        create_lua(lua_mod, init_flags)
    }

    /// Loads the specified set of safe standard libraries into an existing Lua state.
    ///
    /// Use the [`StdLib`] flags to specifiy the libraries you want to load.
    ///
    /// Note that the `debug` library can't be loaded using this function as it can be used to break
    /// the safety guarantees of rlua.  If you really want to load it, use the sister function
    /// [`Lua::unsafe_load_from_std_lib`].
    ///
    /// # Panics
    ///
    /// Panics if `lua_mod` contains `StdLib::DEBUG`
    pub fn load_from_std_lib(&self, lua_mod: StdLib) -> Result<()> {
        assert!(
            !lua_mod.contains(StdLib::DEBUG),
            "The lua debug module can't be loaded using `load_from_std_lib`. Use `unsafe_load_from_std_lib` instead."
        );

        unsafe {
            protect_lua_closure(self.main_state, 0, 0, |state| {
                load_from_std_lib(state, lua_mod);
            })
        }
    }

    /// Loads the specified set of standard libraries into an existing Lua state.
    ///
    /// Use the [`StdLib`] flags to specifiy the libraries you want to load.
    ///
    /// This function is unsafe because it can be used to load the `debug` library which can be used
    /// to break the safety guarantees provided by rlua.
    pub unsafe fn unsafe_load_from_std_lib(&self, lua_mod: StdLib) -> Result<()> {
        protect_lua_closure(self.main_state, 0, 0, |state| {
            load_from_std_lib(state, lua_mod);
        })
    }

    /// The main entry point of the rlua API.
    ///
    /// In order to create Lua values, load and execute Lua code, or otherwise interact with the Lua
    /// state in any way, you must first call `Lua::context` and then call methods on the provided
    /// [`Context`] parameter.
    ///
    /// rlua uses reference types like `String` and `Table` which reference shared data in the Lua
    /// state.  These are special reference counted types that contain pointers to the main Lua
    /// state via the [`Context`] type, and there is a `'lua` lifetime associated with these.
    ///
    /// This `'lua` lifetime is somewhat special.  It is what is sometimes called a "generative"
    /// lifetime or a "branding" lifetime, which is invariant, and unique for each call to
    /// `Lua::context`.
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
    /// callback, or store them outside of the callback in any way.  There is an escape hatch here,
    /// though: if you need to keep references to internal Lua values long-term, you can use the Lua
    /// registry via [`Context::set_named_registry_value`] and [`Context::create_registry_value`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use rlua::{Lua, Result};
    /// # fn main() -> Result<()> {
    /// let lua = Lua::new();
    /// lua.context(|lua_context| {
    ///    lua_context.load(r#"
    ///        print("hello world!")
    ///    "#).exec()
    /// })?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`Context`]: struct.Context.html
    /// [`Context::set_named_registry_value`]: struct.Context.html#method.set_named_registry_value
    /// [`Context::create_registry_value`]: struct.Context.html#method.create_registry_value
    pub fn context<F, R>(&self, f: F) -> R
    where
        F: FnOnce(Context) -> R,
    {
        f(unsafe { Context::new(self.main_state) })
    }

    /// Sets a 'hook' function that will periodically be called as Lua code executes.
    ///
    /// When exactly the hook function is called depends on the contents of the `triggers`
    /// parameter, see [`HookTriggers`] for more details.
    ///
    /// The provided hook function can error, and this error will be propagated through the Lua code
    /// that was executing at the time the hook was triggered.  This can be used to implement a
    /// limited form of execution limits by setting [`HookTriggers.every_nth_instruction`] and
    /// erroring once an instruction limit has been reached.
    ///
    /// # Example
    ///
    /// Shows each line number of code being executed by the Lua interpreter.
    ///
    /// ```
    /// # use rlua::{Lua, HookTriggers, Result};
    /// # fn main() -> Result<()> {
    /// let lua = Lua::new();
    /// lua.set_hook(HookTriggers {
    ///     every_line: true, ..Default::default()
    /// }, |_lua_context, debug| {
    ///     println!("line {}", debug.curr_line());
    ///     Ok(())
    /// });
    /// lua.context(|lua_context| {
    ///     lua_context.load(r#"
    ///         local x = 2 + 3
    ///         local y = x * 63
    ///         local z = string.len(x..", "..y)
    ///     "#).exec()
    /// })?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`HookTriggers`]: struct.HookTriggers.html
    /// [`HookTriggers.every_nth_instruction`]: struct.HookTriggers.html#field.every_nth_instruction
    pub fn set_hook<F>(&self, triggers: HookTriggers, callback: F)
    where
        F: 'static + Send + FnMut(Context, Debug) -> Result<()>,
    {
        unsafe {
            (*extra_data(self.main_state)).hook_callback = Some(Rc::new(RefCell::new(callback)));
            ffi::lua_sethook(
                self.main_state,
                Some(hook_proc),
                triggers.mask(),
                triggers.count(),
            );
        }
    }

    /// Remove any hook previously set by `set_hook`. This function has no effect if a hook was not
    /// previously set.
    pub fn remove_hook(&self) {
        unsafe {
            (*extra_data(self.main_state)).hook_callback = None;
            ffi::lua_sethook(self.main_state, None, 0, 0);
        }
    }

    /// Returns the memory currently used inside this Lua state.
    pub fn used_memory(&self) -> usize {
        unsafe { (*extra_data(self.main_state)).used_memory }
    }

    /// Sets a memory limit on this Lua state.  Once an allocation occurs that would pass this
    /// memory limit, a `Error::MemoryError` is generated instead.
    pub fn set_memory_limit(&self, memory_limit: Option<usize>) {
        unsafe {
            (*extra_data(self.main_state)).memory_limit = memory_limit;
        }
    }

    #[cfg(any(rlua_lua53, rlua_lua54))]
    /// Returns true if the garbage collector is currently running automatically.
    pub fn gc_is_running(&self) -> bool {
        unsafe { ffi::lua_gc(self.main_state, ffi::LUA_GCISRUNNING, 0) != 0 }
    }

    /// Stop the Lua GC from running
    pub fn gc_stop(&self) {
        unsafe {
            ffi::lua_gc(self.main_state, ffi::LUA_GCSTOP, 0);
        }
    }

    /// Restarts the Lua GC if it is not running
    pub fn gc_restart(&self) {
        unsafe {
            ffi::lua_gc(self.main_state, ffi::LUA_GCRESTART, 0);
        }
    }

    /// Perform a full garbage-collection cycle.
    ///
    /// It may be necessary to call this function twice to collect all currently unreachable
    /// objects.  Once to finish the current gc cycle, and once to start and finish the next cycle.
    pub fn gc_collect(&self) -> Result<()> {
        unsafe {
            protect_lua_closure(self.main_state, 0, 0, |state| {
                ffi::lua_gc(state, ffi::LUA_GCCOLLECT, 0);
            })
        }
    }

    /// Steps the garbage collector one indivisible step.
    ///
    /// Returns true if this has finished a collection cycle.
    pub fn gc_step(&self) -> Result<bool> {
        self.gc_step_kbytes(0)
    }

    /// Steps the garbage collector as though memory had been allocated.
    ///
    /// if `kbytes` is 0, then this is the same as calling `gc_step`.  Returns true if this step has
    /// finished a collection cycle.
    pub fn gc_step_kbytes(&self, kbytes: c_int) -> Result<bool> {
        unsafe {
            protect_lua_closure(self.main_state, 0, 0, |state| {
                ffi::lua_gc(state, ffi::LUA_GCSTEP, kbytes) != 0
            })
        }
    }

    #[cfg(rlua_lua54)]
    /// Sets the garbage collector to incremental mode.
    ///
    /// Returns the previous mode (`LUA_GCGEN` or `LUA_GCINC`).  More information can be found in the
    /// [Lua 5.4 documentation][lua_doc].
    ///
    /// [lua_doc]: https://www.lua.org/manual/5.4/manual.html#2.5
    pub fn gc_set_inc(&self, pause: c_int, step_multiplier: c_int, step_size: c_int) -> c_int {
        unsafe {
            ffi::lua_gc(
                self.main_state,
                ffi::LUA_GCINC,
                pause,
                step_multiplier,
                step_size,
            )
        }
    }

    #[cfg(rlua_lua54)]
    /// Sets the garbage collector to generational mode.
    ///
    /// Returns the previous mode (`LUA_GCGEN` or `LUA_GCINC`).  More information can be found in the
    /// [Lua 5.4 documentation][lua_doc].
    ///
    /// [lua_doc]: https://www.lua.org/manual/5.4/manual.html#2.5
    pub fn gc_set_gen(&self, minor_multiplier: c_int, major_multiplier: c_int) -> c_int {
        unsafe {
            ffi::lua_gc(
                self.main_state,
                ffi::LUA_GCGEN,
                minor_multiplier,
                major_multiplier,
            )
        }
    }

    /// Sets the 'pause' value of the incremental collector.
    ///
    /// Returns the previous value of 'pause'.  More information can be found in the [Lua 5.4
    /// documentation][lua_doc].
    ///
    /// [lua_doc]: https://www.lua.org/manual/5.4/manual.html#2.5
    #[cfg_attr(rlua_lua54, deprecated(note = "please use `gc_set_inc` instead"))]
    #[allow(deprecated)]
    pub fn gc_set_pause(&self, pause: c_int) -> c_int {
        unsafe { ffi::lua_gc(self.main_state, ffi::LUA_GCSETPAUSE, pause) }
    }

    /// Sets the 'step multiplier' value of the incremental collector.
    ///
    /// Returns the previous value of the 'step multiplier'.  More information can be found in the
    /// [Lua 5.4 documentation][lua_doc].
    ///
    /// [lua_doc]: https://www.lua.org/manual/5.4/manual.html#2.5
    #[cfg_attr(rlua_lua54, deprecated(note = "please use `gc_set_inc` instead"))]
    #[allow(deprecated)]
    pub fn gc_set_step_multiplier(&self, step_multiplier: c_int) -> c_int {
        unsafe { ffi::lua_gc(self.main_state, ffi::LUA_GCSETSTEPMUL, step_multiplier) }
    }
}

impl Default for Lua {
    fn default() -> Lua {
        Lua::new()
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

    used_memory: usize,
    memory_limit: Option<usize>,

    pub hook_callback: Option<Rc<RefCell<dyn FnMut(Context, Debug) -> Result<()>>>>,
    pub ud:Option<*mut std::ffi::c_void>,
    pub uf:ffi::lua_Alloc,
}

// Return the extra data pointer passed to `lua_newstate()`.  `state` must
// be the main state, not a substate.
pub(crate) unsafe fn extra_data(state: *mut ffi::lua_State) -> *mut ExtraData {
    #[cfg(any(rlua_lua53, rlua_lua54))]
    return *(ffi::lua_getextraspace(state) as *mut *mut ExtraData);
    #[cfg(rlua_lua51)]
    {
        let mut extra: *mut c_void = ptr::null_mut();
        let _ = ffi::lua_getallocf(state, &mut extra);
        return extra as *mut ExtraData;
    }
}

unsafe fn create_lua(lua_mod_to_load: StdLib, init_flags: InitFlags) -> Lua {
    unsafe extern "C" fn allocator(
        extra_data: *mut c_void,
        ptr: *mut c_void,
        osize: usize,
        nsize: usize,
    ) -> *mut c_void {
        let extra_data = extra_data as *mut ExtraData;

        if cfg!(rlua_luajit) {
            ((*extra_data).uf.unwrap())((*extra_data).ud.unwrap(), ptr, osize, nsize)
        } else {
            // If the `ptr` argument is null, osize instead encodes the allocated object type, which
            // we currently ignore.
            let new_used_memory = if ptr.is_null() {
                (*extra_data).used_memory + nsize
            } else if nsize >= osize {
                (*extra_data).used_memory + (nsize - osize)
            } else {
                (*extra_data).used_memory - (osize - nsize)
            };

            if new_used_memory > (*extra_data).used_memory {
                // We only check memory limits when memory is allocated, not freed
                if let Some(memory_limit) = (*extra_data).memory_limit {
                    if new_used_memory > memory_limit {
                        return ptr::null_mut();
                    }
                }
            }

            if nsize == 0 {
                (*extra_data).used_memory = new_used_memory;
                libc::free(ptr as *mut libc::c_void);
                ptr::null_mut()
            } else {
                let p = libc::realloc(ptr as *mut libc::c_void, nsize) as *mut c_void;
                if !p.is_null() {
                    // Only commit the new used memory if the allocation was successful.  Probably in
                    // reality, libc::realloc will never fail.
                    (*extra_data).used_memory = new_used_memory;
                }
                p
            }
        }
    }

    let mut extra = Box::new(ExtraData {
        registered_userdata: HashMap::new(),
        registry_unref_list: Arc::new(Mutex::new(Some(Vec::new()))),
        ref_thread: ptr::null_mut(),
        // We need 1 extra stack space to move values in and out of the ref stack.
        ref_stack_size: ffi::LUA_MINSTACK - 1,
        ref_stack_max: 0,
        ref_free: Vec::new(),
        used_memory: 0,
        memory_limit: None,
        hook_callback: None,
        ud:None,
        uf:None,
    });

    let state = if cfg!(rlua_luajit) {
        let state = ffi::luaL_newstate();
        let mut ud = std::ptr::null_mut();
        let alloc = ffi::lua_getallocf(state, &mut ud as _);
        extra.ud = Some(ud);
        extra.uf = alloc;
        ffi::lua_setallocf(state, Some(allocator), &mut * extra as *mut ExtraData as *mut c_void);
        state
    } else {
        let state = ffi::lua_newstate(
            Some(allocator),
            &mut *extra as *mut ExtraData as *mut c_void,
        );
        state
    };

    #[cfg(rlua_lua54)]
    ffi::lua_setcstacklimit(state, SAFE_CSTACK_SIZE);

    extra.ref_thread = rlua_expect!(
        protect_lua_closure(state, 0, 0, |state| {
            load_from_std_lib(state, lua_mod_to_load);

            init_error_registry(state, init_flags.contains(InitFlags::PCALL_WRAPPERS));

            // Create the function metatable

            ffi::lua_pushlightuserdata(
                state,
                &FUNCTION_METATABLE_REGISTRY_KEY as *const u8 as *mut c_void,
            );

            ffi::lua_newtable(state);

            ffi::lua_pushstring(state, cstr!("__gc"));
            ffi::lua_pushcfunction(state, Some(userdata_destructor::<Callback>));
            ffi::lua_rawset(state, -3);

            ffi::lua_pushstring(state, cstr!("__metatable"));
            ffi::lua_pushboolean(state, 0);
            ffi::lua_rawset(state, -3);

            ffi::lua_rawset(state, ffi::LUA_REGISTRYINDEX);

            // Override pcall and xpcall with versions that cannot be used to catch rust panics.
            if init_flags.contains(InitFlags::PCALL_WRAPPERS) {
                push_globaltable(state);

                ffi::lua_pushstring(state, cstr!("pcall"));
                ffi::lua_pushcfunction(state, Some(safe_pcall));
                ffi::lua_rawset(state, -3);

                ffi::lua_pushstring(state, cstr!("xpcall"));
                ffi::lua_pushcfunction(state, Some(safe_xpcall));
                ffi::lua_rawset(state, -3);

                ffi::lua_pop(state, 1);
            }

            // Override dofile, load, and loadfile with versions that won't load
            // binary files.
            if init_flags.contains(InitFlags::LOAD_WRAPPERS) {
                // These are easier to override in Lua.
                #[cfg(any(rlua_lua53, rlua_lua54))]
                let wrapload = r#"
                    do
                        -- load(chunk [, chunkname [, mode [, env]]])
                        local real_load = load
                        load = function(...)
                            local args = table.pack(...)
                            args[3] = "t"
                            if args.n < 3 then args.n = 3 end
                            return real_load(table.unpack(args))
                        end

                        -- loadfile ([filename [, mode [, env]]])
                        local real_loadfile = loadfile
                        local real_error = error
                        loadfile = function(...)
                            local args = table.pack(...)
                            args[2] = "t"
                            if args.n < 2 then args.n = 2 end
                            return real_loadfile(table.unpack(args))
                        end

                        -- dofile([filename])
                        local real_dofile = dofile
                        dofile = function(filename)
                            -- Note: this is the wrapped loadfile above
                            local chunk = loadfile(filename)
                            if chunk then
                                return chunk()
                            else
                                real_error("rlua dofile: attempt to load bytecode")
                            end
                        end
                    end
                "#;
                #[cfg(rlua_lua51)]
                let wrapload = r#"
                    do
                        -- load(chunk [, chunkname])
                        local real_load = load
                        -- save type() in case user code replaces it
                        local real_type = type
                        local real_error = error
                        load = function(func, chunkname) 
                            local first_chunk = true
                            local wrap_func = function()
                                if not first_chunk then
                                    return func()
                                else
                                    local data = func()
                                    if data == nil then return nil end
                                    assert(real_type(data) == "string")
                                    if data:len() > 0 then
                                        if data:byte(1) == 27 then
                                            real_error("rlua load: loading binary chunks is not allowed")
                                        end
                                        first_chunk = false
                                    end
                                    return data
                                end
                            end
                            return real_load(wrap_func, chunkname)
                        end

                        -- loadstring(string [, chunkname])
                        local real_loadstring = loadstring
                        loadstring = function(s, chunkname)
                            if type(s) ~= "string" then
                                real_error("rlua loadstring: string expected.")
                            elseif s:byte(1) == 27 then
                                -- This is a binary chunk, so disallow
                                return nil, "rlua loadstring: loading binary chunks is not allowed"
                            else
                                return real_loadstring(s, chunkname)
                            end
                        end

                        -- loadfile ([filename])
                        local real_loadfile = loadfile
                        local real_io_open = io.open
                        loadfile = function(filename)
                            local f, err = real_io_open(filename, "rb")
                            if not f then
                                return nil, err
                            end
                            local first_chunk = true
                            local func = function()
                                return f:read(4096)
                            end
                            -- Note: the safe load from above.
                            return load(func, filename)
                        end

                        -- dofile([filename])
                        local real_dofile = dofile
                        dofile = function(filename)
                            -- Note: this is the wrapped loadfile above
                            local chunk = loadfile(filename)
                            if chunk then
                                return chunk()
                            else
                                real_error("rlua dofile: attempt to load bytecode")
                            end
                        end
                    end
                "#;

                let result = dostring(state, wrapload);
                if result != 0 {
                    use std::ffi::CStr;
                    let errmsg = ffi::lua_tostring(state, -1);
                    eprintln!(
                        "Internal error running setup code: {:?}",
                        CStr::from_ptr(errmsg)
                    );
                }
                assert_eq!(result, 0);
            }

            if init_flags.contains(InitFlags::REMOVE_LOADLIB) {
                ffi::lua_getglobal(state, cstr!("package"));
                let t = ffi::lua_type(state, -1);
                if t == ffi::LUA_TTABLE {
                    // Package is loaded.  Remove loadlib.
                    ffi::lua_pushnil(state);
                    ffi::lua_setfield(state, -2, cstr!("loadlib"));

                    #[cfg(rlua_lua51)]
                    let searchers_name = cstr!("loaders");
                    #[cfg(any(rlua_lua53, rlua_lua54))]
                    let searchers_name = cstr!("searchers");

                    ffi::lua_getfield(state, -1, searchers_name);
                    debug_assert_eq!(ffi::lua_type(state, -1), ffi::LUA_TTABLE);
                    debug_assert_eq!(rawlen(state, -1), 4);
                    // Remove the searchers/loaders which will load C libraries.
                    ffi::lua_pushnil(state);
                    ffi::lua_rawseti(state, -2, 4);
                    ffi::lua_pushnil(state);
                    ffi::lua_rawseti(state, -2, 3);

                    ffi::lua_pop(state, 1);
                } else {
                    // Assume it's not present otherwise.
                    assert_eq!(t, ffi::LUA_TNIL);
                }
                // Pop the package (or nil) off the stack.
                ffi::lua_pop(state, 1);
            }

            // Create ref stack thread and place it in the registry to prevent it from being garbage
            // collected.

            let ref_thread = ffi::lua_newthread(state);
            ffi::luaL_ref(state, ffi::LUA_REGISTRYINDEX);
            ref_thread
        }),
        "Error during Lua construction",
    );

    rlua_debug_assert!(ffi::lua_gettop(state) == 0, "stack leak during creation");
    assert_stack(state, ffi::LUA_MINSTACK as i32);

    #[cfg(any(rlua_lua53, rlua_lua54))]
    {
        // Place pointer to ExtraData in the lua_State "extra space"
        *(ffi::lua_getextraspace(state) as *mut *mut ExtraData) = Box::into_raw(extra);
    }
    #[cfg(rlua_lua51)]
    // Prevent extra from being deallocated
    let _ = Box::into_raw(extra);

    Lua {
        main_state: state,
        _no_ref_unwind_safe: PhantomData,
    }
}

unsafe fn load_from_std_lib(state: *mut ffi::lua_State, lua_mod: StdLib) {
    if lua_mod.contains(StdLib::BASE) {
        requiref(state, cstr!("_G"), Some(ffi::luaopen_base), 1);
    }
    #[cfg(any(rlua_lua53, rlua_lua54))]
    if lua_mod.contains(StdLib::COROUTINE) {
        requiref(state, cstr!("coroutine"), Some(ffi::luaopen_coroutine), 1);
    }
    if lua_mod.contains(StdLib::TABLE) {
        requiref(state, cstr!("table"), Some(ffi::luaopen_table), 1);
    }
    if lua_mod.contains(StdLib::IO) {
        requiref(state, cstr!("io"), Some(ffi::luaopen_io), 1);
    }
    #[cfg(not(feature = "lua-no-oslib"))]
    if lua_mod.contains(StdLib::OS) {
        requiref(state, cstr!("os"), Some(ffi::luaopen_os), 1);
    }
    if lua_mod.contains(StdLib::STRING) {
        requiref(state, cstr!("string"), Some(ffi::luaopen_string), 1);
    }
    #[cfg(any(rlua_lua53, rlua_lua54))]
    if lua_mod.contains(StdLib::UTF8) {
        requiref(state, cstr!("utf8"), Some(ffi::luaopen_utf8), 1);
    }
    if lua_mod.contains(StdLib::MATH) {
        requiref(state, cstr!("math"), Some(ffi::luaopen_math), 1);
    }
    if lua_mod.contains(StdLib::PACKAGE) {
        requiref(state, cstr!("package"), Some(ffi::luaopen_package), 1);
    }
    if lua_mod.contains(StdLib::DEBUG) {
        requiref(state, cstr!("debug"), Some(ffi::luaopen_debug), 1);
    }
}

pub(crate) static FUNCTION_METATABLE_REGISTRY_KEY: u8 = 0;
