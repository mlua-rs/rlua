use std::{ptr, str};
use std::ops::DerefMut;
use std::cell::RefCell;
use std::ffi::CString;
use std::any::TypeId;
use std::marker::PhantomData;
use std::collections::HashMap;
use std::os::raw::{c_char, c_int, c_void};
use std::process;

use libc;

use ffi;
use error::*;
use util::*;
use value::{FromLua, FromLuaMulti, MultiValue, Nil, ToLua, ToLuaMulti, Value};
use types::{Callback, Integer, LightUserData, LuaRef, Number};
use string::String;
use table::Table;
use function::Function;
use thread::Thread;
use userdata::{AnyUserData, MetaMethod, UserData, UserDataMethods};

/// Top level Lua struct which holds the Lua state itself.
pub struct Lua {
    pub(crate) state: *mut ffi::lua_State,
    main_state: *mut ffi::lua_State,
    ephemeral: bool,
}

impl Drop for Lua {
    fn drop(&mut self) {
        unsafe {
            if !self.ephemeral {
                if cfg!(test) {
                    let top = ffi::lua_gettop(self.state);
                    if top != 0 {
                        eprintln!("Lua stack leak detected, stack top is {}", top);
                        process::abort()
                    }
                }

                ffi::lua_close(self.state);
            }
        }
    }
}

impl Lua {
    /// Creates a new Lua state and loads standard library without the `debug` library.
    pub fn new() -> Lua {
        unsafe { Lua::create_lua(false) }
    }

    /// Creates a new Lua state and loads the standard library including the `debug` library.
    ///
    /// The debug library is very unsound, loading it and using it breaks all the guarantees of
    /// rlua.
    pub unsafe fn new_with_debug() -> Lua {
        Lua::create_lua(true)
    }

    /// Loads a chunk of Lua code and returns it as a function.
    ///
    /// The source can be named by setting the `name` parameter. This is generally recommended as it
    /// results in better error traces.
    ///
    /// Equivalent to Lua's `load` function.
    pub fn load(&self, source: &str, name: Option<&str>) -> Result<Function> {
        unsafe {
            stack_err_guard(self.state, 0, || {
                check_stack(self.state, 1);

                match if let Some(name) = name {
                    let name =
                        CString::new(name.to_owned()).map_err(|e| Error::ToLuaConversionError {
                            from: "&str",
                            to: "string",
                            message: Some(e.to_string()),
                        })?;
                    ffi::luaL_loadbuffer(
                        self.state,
                        source.as_ptr() as *const c_char,
                        source.len(),
                        name.as_ptr(),
                    )
                } else {
                    ffi::luaL_loadbuffer(
                        self.state,
                        source.as_ptr() as *const c_char,
                        source.len(),
                        ptr::null(),
                    )
                } {
                    ffi::LUA_OK => Ok(Function(self.pop_ref(self.state))),
                    err => Err(pop_error(self.state, err)),
                }
            })
        }
    }

    /// Execute a chunk of Lua code.
    ///
    /// This is equivalent to simply loading the source with `load` and then calling the resulting
    /// function with no arguments.
    ///
    /// Returns the values returned by the chunk.
    pub fn exec<'lua, R: FromLuaMulti<'lua>>(
        &'lua self,
        source: &str,
        name: Option<&str>,
    ) -> Result<R> {
        self.load(source, name)?.call(())
    }

    /// Evaluate the given expression or chunk inside this Lua state.
    ///
    /// If `source` is an expression, returns the value it evaluates to. Otherwise, returns the
    /// values returned by the chunk (if any).
    pub fn eval<'lua, R: FromLuaMulti<'lua>>(
        &'lua self,
        source: &str,
        name: Option<&str>,
    ) -> Result<R> {
        // First, try interpreting the lua as an expression by adding
        // "return", then as a statement.  This is the same thing the
        // actual lua repl does.
        self.load(&format!("return {}", source), name)
            .or_else(|_| self.load(source, name))?
            .call(())
    }

    /// Pass a `&str` slice to Lua, creating and returning an interned Lua string.
    pub fn create_string(&self, s: &str) -> Result<String> {
        unsafe {
            stack_err_guard(self.state, 0, || {
                check_stack(self.state, 4);
                push_string(self.state, s)?;
                Ok(String(self.pop_ref(self.state)))
            })
        }
    }

    /// Creates and returns a new table.
    pub fn create_table(&self) -> Result<Table> {
        unsafe {
            stack_err_guard(self.state, 0, || {
                check_stack(self.state, 4);
                protect_lua_call(self.state, 0, 1, |state| {
                    ffi::lua_newtable(state);
                })?;
                Ok(Table(self.pop_ref(self.state)))
            })
        }
    }

    /// Creates a table and fills it with values from an iterator.
    pub fn create_table_from<'lua, K, V, I>(&'lua self, cont: I) -> Result<Table<'lua>>
    where
        K: ToLua<'lua>,
        V: ToLua<'lua>,
        I: IntoIterator<Item = (K, V)>,
    {
        unsafe {
            stack_err_guard(self.state, 0, || {
                check_stack(self.state, 6);
                protect_lua_call(self.state, 0, 1, |state| {
                    ffi::lua_newtable(state);
                })?;

                for (k, v) in cont {
                    self.push_value(self.state, k.to_lua(self)?);
                    self.push_value(self.state, v.to_lua(self)?);
                    protect_lua_call(self.state, 3, 1, |state| {
                        ffi::lua_rawset(state, -3);
                    })?;
                }
                Ok(Table(self.pop_ref(self.state)))
            })
        }
    }

    /// Creates a table from an iterator of values, using `1..` as the keys.
    pub fn create_sequence_from<'lua, T, I>(&'lua self, cont: I) -> Result<Table<'lua>>
    where
        T: ToLua<'lua>,
        I: IntoIterator<Item = T>,
    {
        self.create_table_from(cont.into_iter().enumerate().map(|(k, v)| (k + 1, v)))
    }

    /// Wraps a Rust function or closure, creating a callable Lua function handle to it.
    ///
    /// # Examples
    ///
    /// Create a function which prints its argument:
    ///
    /// ```
    /// # extern crate rlua;
    /// # use rlua::{Lua, Result};
    /// # fn try_main() -> Result<()> {
    /// let lua = Lua::new();
    ///
    /// let greet = lua.create_function(|_, name: String| {
    ///     println!("Hello, {}!", name);
    ///     Ok(())
    /// });
    /// # let _ = greet;    // used
    /// # Ok(())
    /// # }
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    ///
    /// Use tuples to accept multiple arguments:
    ///
    /// ```
    /// # extern crate rlua;
    /// # use rlua::{Lua, Result};
    /// # fn try_main() -> Result<()> {
    /// let lua = Lua::new();
    ///
    /// let print_person = lua.create_function(|_, (name, age): (String, u8)| {
    ///     println!("{} is {} years old!", name, age);
    ///     Ok(())
    /// });
    /// # let _ = print_person;    // used
    /// # Ok(())
    /// # }
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    pub fn create_function<'lua, A, R, F>(&'lua self, mut func: F) -> Result<Function<'lua>>
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        F: 'static + FnMut(&'lua Lua, A) -> Result<R>,
    {
        self.create_callback_function(Box::new(move |lua, args| {
            func(lua, A::from_lua_multi(args, lua)?)?.to_lua_multi(lua)
        }))
    }

    /// Wraps a Lua function into a new thread (or coroutine).
    ///
    /// Equivalent to `coroutine.create`.
    pub fn create_thread<'lua>(&'lua self, func: Function<'lua>) -> Result<Thread<'lua>> {
        unsafe {
            stack_err_guard(self.state, 0, move || {
                check_stack(self.state, 2);

                let thread_state =
                    protect_lua_call(self.state, 0, 1, |state| ffi::lua_newthread(state))?;
                self.push_ref(thread_state, &func.0);

                Ok(Thread(self.pop_ref(self.state)))
            })
        }
    }

    /// Create a Lua userdata object from a custom userdata type.
    pub fn create_userdata<T>(&self, data: T) -> Result<AnyUserData>
    where
        T: UserData,
    {
        unsafe {
            stack_err_guard(self.state, 0, move || {
                check_stack(self.state, 3);

                push_userdata::<RefCell<T>>(self.state, RefCell::new(data))?;

                ffi::lua_rawgeti(
                    self.state,
                    ffi::LUA_REGISTRYINDEX,
                    self.userdata_metatable::<T>()? as ffi::lua_Integer,
                );

                ffi::lua_setmetatable(self.state, -2);

                Ok(AnyUserData(self.pop_ref(self.state)))
            })
        }
    }

    /// Returns a handle to the global environment.
    pub fn globals(&self) -> Table {
        unsafe {
            stack_guard(self.state, 0, move || {
                check_stack(self.state, 2);
                ffi::lua_rawgeti(self.state, ffi::LUA_REGISTRYINDEX, ffi::LUA_RIDX_GLOBALS);
                Table(self.pop_ref(self.state))
            })
        }
    }

    /// Coerces a Lua value to a string.
    ///
    /// The value must be a string (in which case this is a no-op) or a number.
    pub fn coerce_string<'lua>(&'lua self, v: Value<'lua>) -> Result<String<'lua>> {
        match v {
            Value::String(s) => Ok(s),
            v => unsafe {
                stack_err_guard(self.state, 0, || {
                    check_stack(self.state, 2);
                    let ty = v.type_name();
                    self.push_value(self.state, v);
                    let s =
                        protect_lua_call(self.state, 1, 1, |state| ffi::lua_tostring(state, -1))?;
                    if s.is_null() {
                        ffi::lua_pop(self.state, 1);
                        Err(Error::FromLuaConversionError {
                            from: ty,
                            to: "String",
                            message: Some("expected string or number".to_string()),
                        })
                    } else {
                        Ok(String(self.pop_ref(self.state)))
                    }
                })
            },
        }
    }

    /// Coerces a Lua value to an integer.
    ///
    /// The value must be an integer, or a floating point number or a string that can be converted
    /// to an integer. Refer to the Lua manual for details.
    pub fn coerce_integer(&self, v: Value) -> Result<Integer> {
        match v {
            Value::Integer(i) => Ok(i),
            v => unsafe {
                stack_guard(self.state, 0, || {
                    check_stack(self.state, 1);
                    let ty = v.type_name();
                    self.push_value(self.state, v);
                    let mut isint = 0;
                    let i = ffi::lua_tointegerx(self.state, -1, &mut isint);
                    ffi::lua_pop(self.state, 1);
                    if isint == 0 {
                        Err(Error::FromLuaConversionError {
                            from: ty,
                            to: "integer",
                            message: None,
                        })
                    } else {
                        Ok(i)
                    }
                })
            },
        }
    }

    /// Coerce a Lua value to a number.
    ///
    /// The value must be a number or a string that can be converted to a number. Refer to the Lua
    /// manual for details.
    pub fn coerce_number(&self, v: Value) -> Result<Number> {
        match v {
            Value::Number(n) => Ok(n),
            v => unsafe {
                stack_guard(self.state, 0, || {
                    check_stack(self.state, 1);
                    let ty = v.type_name();
                    self.push_value(self.state, v);
                    let mut isnum = 0;
                    let n = ffi::lua_tonumberx(self.state, -1, &mut isnum);
                    ffi::lua_pop(self.state, 1);
                    if isnum == 0 {
                        Err(Error::FromLuaConversionError {
                            from: ty,
                            to: "number",
                            message: Some("number or string coercible to number".to_string()),
                        })
                    } else {
                        Ok(n)
                    }
                })
            },
        }
    }

    /// Converts a value that implements `ToLua` into a `Value` instance.
    pub fn pack<'lua, T: ToLua<'lua>>(&'lua self, t: T) -> Result<Value<'lua>> {
        t.to_lua(self)
    }

    /// Converts a `Value` instance into a value that implements `FromLua`.
    pub fn unpack<'lua, T: FromLua<'lua>>(&'lua self, value: Value<'lua>) -> Result<T> {
        T::from_lua(value, self)
    }

    /// Converts a value that implements `ToLuaMulti` into a `MultiValue` instance.
    pub fn pack_multi<'lua, T: ToLuaMulti<'lua>>(&'lua self, t: T) -> Result<MultiValue<'lua>> {
        t.to_lua_multi(self)
    }

    /// Converts a `MultiValue` instance into a value that implements `FromLuaMulti`.
    pub fn unpack_multi<'lua, T: FromLuaMulti<'lua>>(
        &'lua self,
        value: MultiValue<'lua>,
    ) -> Result<T> {
        T::from_lua_multi(value, self)
    }

    /// Set a value in the Lua registry based on a string key.
    ///
    /// This value will be available to rust from all `Lua` instances which share the same main
    /// state.
    pub fn set_registry<'lua, T: ToLua<'lua>>(&'lua self, registry_key: &str, t: T) -> Result<()> {
        unsafe {
            stack_err_guard(self.state, 0, || {
                check_stack(self.state, 5);
                push_string(self.state, registry_key)?;
                self.push_value(self.state, t.to_lua(self)?);
                protect_lua_call(self.state, 2, 0, |state| {
                    ffi::lua_settable(state, ffi::LUA_REGISTRYINDEX);
                })
            })
        }
    }

    /// Get a value from the Lua registry based on a string key.
    ///
    /// Any Lua instance which shares the underlying main state may call `get_registry` to get a
    /// value previously set by `set_registry`.
    pub fn get_registry<'lua, T: FromLua<'lua>>(&'lua self, registry_key: &str) -> Result<T> {
        unsafe {
            stack_err_guard(self.state, 0, || {
                check_stack(self.state, 4);
                push_string(self.state, registry_key)?;
                protect_lua_call(self.state, 1, 1, |state| {
                    ffi::lua_gettable(state, ffi::LUA_REGISTRYINDEX)
                })?;
                T::from_lua(self.pop_value(self.state), self)
            })
        }
    }

    // Uses 1 stack space, does not call checkstack
    pub(crate) unsafe fn push_value(&self, state: *mut ffi::lua_State, value: Value) {
        match value {
            Value::Nil => {
                ffi::lua_pushnil(state);
            }

            Value::Boolean(b) => {
                ffi::lua_pushboolean(state, if b { 1 } else { 0 });
            }

            Value::LightUserData(ud) => {
                ffi::lua_pushlightuserdata(state, ud.0);
            }

            Value::Integer(i) => {
                ffi::lua_pushinteger(state, i);
            }

            Value::Number(n) => {
                ffi::lua_pushnumber(state, n);
            }

            Value::String(s) => {
                self.push_ref(state, &s.0);
            }

            Value::Table(t) => {
                self.push_ref(state, &t.0);
            }

            Value::Function(f) => {
                self.push_ref(state, &f.0);
            }

            Value::Thread(t) => {
                self.push_ref(state, &t.0);
            }

            Value::UserData(ud) => {
                self.push_ref(state, &ud.0);
            }

            Value::Error(e) => {
                push_wrapped_error(state, e);
            }
        }
    }

    // Uses 1 stack space, does not call checkstack
    pub(crate) unsafe fn pop_value(&self, state: *mut ffi::lua_State) -> Value {
        match ffi::lua_type(state, -1) {
            ffi::LUA_TNIL => {
                ffi::lua_pop(state, 1);
                Nil
            }

            ffi::LUA_TBOOLEAN => {
                let b = Value::Boolean(ffi::lua_toboolean(state, -1) != 0);
                ffi::lua_pop(state, 1);
                b
            }

            ffi::LUA_TLIGHTUSERDATA => {
                let ud = Value::LightUserData(LightUserData(ffi::lua_touserdata(state, -1)));
                ffi::lua_pop(state, 1);
                ud
            }

            ffi::LUA_TNUMBER => if ffi::lua_isinteger(state, -1) != 0 {
                let i = Value::Integer(ffi::lua_tointeger(state, -1));
                ffi::lua_pop(state, 1);
                i
            } else {
                let n = Value::Number(ffi::lua_tonumber(state, -1));
                ffi::lua_pop(state, 1);
                n
            },

            ffi::LUA_TSTRING => Value::String(String(self.pop_ref(state))),

            ffi::LUA_TTABLE => Value::Table(Table(self.pop_ref(state))),

            ffi::LUA_TFUNCTION => Value::Function(Function(self.pop_ref(state))),

            ffi::LUA_TUSERDATA => {
                // It should not be possible to interact with userdata types
                // other than custom UserData types OR a WrappedError.
                // WrappedPanic should never be able to be caught in lua, so it
                // should never be here.
                if let Some(err) = pop_wrapped_error(state) {
                    Value::Error(err)
                } else {
                    Value::UserData(AnyUserData(self.pop_ref(state)))
                }
            }

            ffi::LUA_TTHREAD => Value::Thread(Thread(self.pop_ref(state))),

            _ => unreachable!("internal error: LUA_TNONE in pop_value"),
        }
    }

    // Used 1 stack space, does not call checkstack
    pub(crate) unsafe fn push_ref(&self, state: *mut ffi::lua_State, lref: &LuaRef) {
        lua_assert!(
            state,
            lref.lua.main_state == self.main_state,
            "Lua instance passed Value created from a different Lua"
        );

        ffi::lua_rawgeti(
            state,
            ffi::LUA_REGISTRYINDEX,
            lref.registry_id as ffi::lua_Integer,
        );
    }

    // Pops the topmost element of the stack and stores a reference to it in the
    // registry.
    //
    // This pins the object, preventing garbage collection until the returned
    // `LuaRef` is dropped.
    //
    // pop_ref uses 1 extra stack space and does not call checkstack
    pub(crate) unsafe fn pop_ref(&self, state: *mut ffi::lua_State) -> LuaRef {
        let registry_id = ffi::luaL_ref(state, ffi::LUA_REGISTRYINDEX);
        LuaRef {
            lua: self,
            registry_id: registry_id,
        }
    }

    pub(crate) unsafe fn userdata_metatable<T: UserData>(&self) -> Result<c_int> {
        // Used if both an __index metamethod is set and regular methods, checks methods table
        // first, then __index metamethod.
        unsafe extern "C" fn meta_index_impl(state: *mut ffi::lua_State) -> c_int {
            check_stack(state, 2);

            ffi::lua_pushvalue(state, -1);
            ffi::lua_gettable(state, ffi::lua_upvalueindex(1));
            if ffi::lua_isnil(state, -1) == 0 {
                ffi::lua_insert(state, -3);
                ffi::lua_pop(state, 2);
                1
            } else {
                ffi::lua_pop(state, 1);
                ffi::lua_pushvalue(state, ffi::lua_upvalueindex(2));
                ffi::lua_insert(state, -3);
                ffi::lua_call(state, 2, 1);
                1
            }
        }

        stack_err_guard(self.state, 0, move || {
            check_stack(self.state, 5);

            ffi::lua_pushlightuserdata(
                self.state,
                &LUA_USERDATA_REGISTRY_KEY as *const u8 as *mut c_void,
            );
            ffi::lua_gettable(self.state, ffi::LUA_REGISTRYINDEX);
            let registered_userdata = get_userdata::<HashMap<TypeId, c_int>>(self.state, -1)?;
            ffi::lua_pop(self.state, 1);

            if let Some(table_id) = (*registered_userdata).get(&TypeId::of::<T>()) {
                return Ok(*table_id);
            }

            let mut methods = UserDataMethods {
                methods: HashMap::new(),
                meta_methods: HashMap::new(),
                _type: PhantomData,
            };
            T::add_methods(&mut methods);

            protect_lua_call(self.state, 0, 1, |state| {
                ffi::lua_newtable(state);
            })?;

            let has_methods = !methods.methods.is_empty();

            if has_methods {
                push_string(self.state, "__index")?;
                protect_lua_call(self.state, 0, 1, |state| {
                    ffi::lua_newtable(state);
                })?;

                for (k, m) in methods.methods {
                    push_string(self.state, &k)?;
                    self.push_value(
                        self.state,
                        Value::Function(self.create_callback_function(m)?),
                    );
                    protect_lua_call(self.state, 3, 1, |state| {
                        ffi::lua_rawset(state, -3);
                    })?;
                }

                protect_lua_call(self.state, 3, 1, |state| {
                    ffi::lua_rawset(state, -3);
                })?;
            }

            for (k, m) in methods.meta_methods {
                if k == MetaMethod::Index && has_methods {
                    push_string(self.state, "__index")?;
                    ffi::lua_pushvalue(self.state, -1);
                    ffi::lua_gettable(self.state, -3);
                    self.push_value(
                        self.state,
                        Value::Function(self.create_callback_function(m)?),
                    );
                    protect_lua_call(self.state, 2, 1, |state| {
                        ffi::lua_pushcclosure(state, meta_index_impl, 2);
                    })?;

                    protect_lua_call(self.state, 3, 1, |state| {
                        ffi::lua_rawset(state, -3);
                    })?;
                } else {
                    let name = match k {
                        MetaMethod::Add => "__add",
                        MetaMethod::Sub => "__sub",
                        MetaMethod::Mul => "__mul",
                        MetaMethod::Div => "__div",
                        MetaMethod::Mod => "__mod",
                        MetaMethod::Pow => "__pow",
                        MetaMethod::Unm => "__unm",
                        MetaMethod::IDiv => "__idiv",
                        MetaMethod::BAnd => "__band",
                        MetaMethod::BOr => "__bor",
                        MetaMethod::BXor => "__bxor",
                        MetaMethod::BNot => "__bnot",
                        MetaMethod::Shl => "__shl",
                        MetaMethod::Shr => "__shr",
                        MetaMethod::Concat => "__concat",
                        MetaMethod::Len => "__len",
                        MetaMethod::Eq => "__eq",
                        MetaMethod::Lt => "__lt",
                        MetaMethod::Le => "__le",
                        MetaMethod::Index => "__index",
                        MetaMethod::NewIndex => "__newindex",
                        MetaMethod::Call => "__call",
                        MetaMethod::ToString => "__tostring",
                    };
                    push_string(self.state, name)?;
                    self.push_value(
                        self.state,
                        Value::Function(self.create_callback_function(m)?),
                    );
                    protect_lua_call(self.state, 3, 1, |state| {
                        ffi::lua_rawset(state, -3);
                    })?;
                }
            }

            push_string(self.state, "__gc")?;
            ffi::lua_pushcfunction(self.state, userdata_destructor::<RefCell<T>>);
            protect_lua_call(self.state, 3, 1, |state| {
                ffi::lua_rawset(state, -3);
            })?;

            push_string(self.state, "__metatable")?;
            ffi::lua_pushboolean(self.state, 0);
            protect_lua_call(self.state, 3, 1, |state| {
                ffi::lua_rawset(state, -3);
            })?;

            let id = ffi::luaL_ref(self.state, ffi::LUA_REGISTRYINDEX);
            (*registered_userdata).insert(TypeId::of::<T>(), id);
            Ok(id)
        })
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
                    eprintln!("Out of memory in Lua allocation, aborting!");
                    process::abort()
                } else {
                    p as *mut c_void
                }
            }
        }

        let state = ffi::lua_newstate(allocator, ptr::null_mut());

        // Ignores or `unwrap()`s 'm' errors, because this is assuming that nothing in the lua
        // standard library will have a `__gc` metamethod error.
        stack_guard(state, 0, || {
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

            if load_debug {
                ffi::luaL_requiref(state, cstr!("debug"), ffi::luaopen_debug, 1);
                ffi::lua_pop(state, 1);
            }

            // Create the userdata registry table

            ffi::lua_pushlightuserdata(
                state,
                &LUA_USERDATA_REGISTRY_KEY as *const u8 as *mut c_void,
            );

            push_userdata::<HashMap<TypeId, c_int>>(state, HashMap::new()).unwrap();

            ffi::lua_newtable(state);

            push_string(state, "__gc").unwrap();
            ffi::lua_pushcfunction(state, userdata_destructor::<HashMap<TypeId, c_int>>);
            ffi::lua_rawset(state, -3);

            ffi::lua_setmetatable(state, -2);

            ffi::lua_rawset(state, ffi::LUA_REGISTRYINDEX);

            // Create the function metatable

            ffi::lua_pushlightuserdata(
                state,
                &FUNCTION_METATABLE_REGISTRY_KEY as *const u8 as *mut c_void,
            );

            ffi::lua_newtable(state);

            push_string(state, "__gc").unwrap();
            ffi::lua_pushcfunction(state, userdata_destructor::<RefCell<Callback>>);
            ffi::lua_rawset(state, -3);

            push_string(state, "__metatable").unwrap();
            ffi::lua_pushboolean(state, 0);
            ffi::lua_rawset(state, -3);

            ffi::lua_rawset(state, ffi::LUA_REGISTRYINDEX);

            // Override pcall, xpcall, and setmetatable with versions that cannot be used to
            // cause unsafety.

            ffi::lua_rawgeti(state, ffi::LUA_REGISTRYINDEX, ffi::LUA_RIDX_GLOBALS);

            push_string(state, "pcall").unwrap();
            ffi::lua_pushcfunction(state, safe_pcall);
            ffi::lua_rawset(state, -3);

            push_string(state, "xpcall").unwrap();
            ffi::lua_pushcfunction(state, safe_xpcall);
            ffi::lua_rawset(state, -3);

            ffi::lua_pop(state, 1);
        });

        Lua {
            state,
            main_state: state,
            ephemeral: false,
        }
    }

    fn create_callback_function<'lua>(&'lua self, func: Callback<'lua>) -> Result<Function<'lua>> {
        unsafe extern "C" fn callback_call_impl(state: *mut ffi::lua_State) -> c_int {
            callback_error(state, || {
                let lua = Lua {
                    state: state,
                    main_state: main_state(state),
                    ephemeral: true,
                };

                let func = get_userdata::<RefCell<Callback>>(state, ffi::lua_upvalueindex(1))?;
                let mut func = (*func)
                    .try_borrow_mut()
                    .map_err(|_| Error::RecursiveCallbackError)?;

                let nargs = ffi::lua_gettop(state);
                let mut args = MultiValue::new();
                check_stack(state, 1);
                for _ in 0..nargs {
                    args.push_front(lua.pop_value(state));
                }

                let results = func.deref_mut()(&lua, args)?;
                let nresults = results.len() as c_int;

                check_stack(state, nresults);

                for r in results {
                    lua.push_value(state, r);
                }

                Ok(nresults)
            })
        }

        unsafe {
            stack_err_guard(self.state, 0, move || {
                check_stack(self.state, 2);

                push_userdata::<RefCell<Callback>>(self.state, RefCell::new(func))?;

                ffi::lua_pushlightuserdata(
                    self.state,
                    &FUNCTION_METATABLE_REGISTRY_KEY as *const u8 as *mut c_void,
                );
                ffi::lua_gettable(self.state, ffi::LUA_REGISTRYINDEX);
                ffi::lua_setmetatable(self.state, -2);

                protect_lua_call(self.state, 1, 1, |state| {
                    ffi::lua_pushcclosure(state, callback_call_impl, 1);
                })?;

                Ok(Function(self.pop_ref(self.state)))
            })
        }
    }
}

static LUA_USERDATA_REGISTRY_KEY: u8 = 0;
static FUNCTION_METATABLE_REGISTRY_KEY: u8 = 0;
