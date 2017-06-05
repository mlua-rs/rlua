use std::fmt;
use std::ops::{Deref, DerefMut};
use std::iter::FromIterator;
use std::cell::{RefCell, Ref, RefMut};
use std::ptr;
use std::mem;
use std::ffi::{CStr, CString};
use std::any::TypeId;
use std::marker::PhantomData;
use std::collections::{HashMap, VecDeque};
use std::collections::hash_map::Entry as HashMapEntry;
use std::os::raw::{c_char, c_int, c_void};

use ffi;
use error::*;
use util::*;

/// A rust-side handle to an internal Lua value.
#[derive(Debug, Clone)]
pub enum LuaValue<'lua> {
    Nil,
    Boolean(bool),
    LightUserData(LightUserData),
    Integer(LuaInteger),
    Number(LuaNumber),
    String(LuaString<'lua>),
    Table(LuaTable<'lua>),
    Function(LuaFunction<'lua>),
    UserData(LuaUserData<'lua>),
    Thread(LuaThread<'lua>),
}
pub use self::LuaValue::Nil as LuaNil;

/// Trait for types convertible to LuaValue
pub trait ToLua<'a> {
    fn to_lua(self, lua: &'a Lua) -> LuaResult<LuaValue<'a>>;
}

/// Trait for types convertible from LuaValue
pub trait FromLua<'a>: Sized {
    fn from_lua(lua_value: LuaValue<'a>, lua: &'a Lua) -> LuaResult<Self>;
}

/// Multiple lua values used for both argument passing and also for multiple return values.
#[derive(Debug, Clone)]
pub struct LuaMultiValue<'lua>(VecDeque<LuaValue<'lua>>);

impl<'lua> LuaMultiValue<'lua> {
    pub fn new() -> LuaMultiValue<'lua> {
        LuaMultiValue(VecDeque::new())
    }
}

impl<'lua> FromIterator<LuaValue<'lua>> for LuaMultiValue<'lua> {
    fn from_iter<I: IntoIterator<Item = LuaValue<'lua>>>(iter: I) -> Self {
        LuaMultiValue(VecDeque::from_iter(iter))
    }
}

impl<'lua> IntoIterator for LuaMultiValue<'lua> {
    type Item = LuaValue<'lua>;
    type IntoIter = <VecDeque<LuaValue<'lua>> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'lua> Deref for LuaMultiValue<'lua> {
    type Target = VecDeque<LuaValue<'lua>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'lua> DerefMut for LuaMultiValue<'lua> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub trait ToLuaMulti<'a> {
    fn to_lua_multi(self, lua: &'a Lua) -> LuaResult<LuaMultiValue<'a>>;
}

pub trait FromLuaMulti<'a>: Sized {
    fn from_lua_multi(values: LuaMultiValue<'a>, lua: &'a Lua) -> LuaResult<Self>;
}

type LuaCallback = Box<for<'lua> FnMut(&'lua Lua, LuaMultiValue<'lua>)
                                       -> LuaResult<LuaMultiValue<'lua>>>;

struct LuaRef<'lua> {
    lua: &'lua Lua,
    registry_id: c_int,
}

impl<'lua> fmt::Debug for LuaRef<'lua> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "LuaRef({})", self.registry_id)
    }
}

impl<'lua> Clone for LuaRef<'lua> {
    fn clone(&self) -> Self {
        unsafe {
            self.lua.push_ref(self.lua.state, self);
            self.lua.pop_ref(self.lua.state)
        }
    }
}

impl<'lua> Drop for LuaRef<'lua> {
    fn drop(&mut self) {
        unsafe {
            ffi::luaL_unref(self.lua.state, ffi::LUA_REGISTRYINDEX, self.registry_id);
        }
    }
}

pub type LuaInteger = ffi::lua_Integer;
pub type LuaNumber = ffi::lua_Number;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct LightUserData(pub *mut c_void);

/// Handle to an an internal lua string
#[derive(Clone, Debug)]
pub struct LuaString<'lua>(LuaRef<'lua>);

impl<'lua> LuaString<'lua> {
    pub fn get(&self) -> LuaResult<&str> {
        let lua = self.0.lua;
        unsafe {
            stack_guard(lua.state, 0, || {
                check_stack(lua.state, 1)?;
                lua.push_ref(lua.state, &self.0);
                assert_eq!(ffi::lua_type(lua.state, -1), ffi::LUA_TSTRING);
                let s = CStr::from_ptr(ffi::lua_tostring(lua.state, -1)).to_str()?;
                ffi::lua_pop(lua.state, 1);
                Ok(s)
            })
        }
    }
}

/// Handle to an an internal lua table
#[derive(Clone, Debug)]
pub struct LuaTable<'lua>(LuaRef<'lua>);

impl<'lua> LuaTable<'lua> {
    pub fn set<K: ToLua<'lua>, V: ToLua<'lua>>(&self, key: K, value: V) -> LuaResult<()> {
        let lua = self.0.lua;
        let key = key.to_lua(lua)?;
        let value = value.to_lua(lua)?;
        unsafe {
            error_guard(lua.state, 0, 0, |state| {
                check_stack(state, 3)?;
                lua.push_ref(state, &self.0);
                lua.push_value(state, key)?;
                lua.push_value(state, value)?;
                ffi::lua_settable(state, -3);
                Ok(())
            })
        }
    }

    pub fn get<K: ToLua<'lua>, V: FromLua<'lua>>(&self, key: K) -> LuaResult<V> {
        let lua = self.0.lua;
        let key = key.to_lua(lua)?;
        unsafe {
            let res = error_guard(lua.state, 0, 0, |state| {
                check_stack(state, 2)?;
                lua.push_ref(state, &self.0);
                lua.push_value(state, key.to_lua(lua)?)?;
                ffi::lua_gettable(state, -2);
                let res = lua.pop_value(state)?;
                ffi::lua_pop(state, 1);
                Ok(res)
            })?;
            V::from_lua(res, lua)
        }
    }

    /// Set a field in the table, without invoking metamethods
    pub fn raw_set<K: ToLua<'lua>, V: ToLua<'lua>>(&self, key: K, value: V) -> LuaResult<()> {
        let lua = self.0.lua;
        unsafe {
            stack_guard(lua.state, 0, || {
                check_stack(lua.state, 3)?;
                lua.push_ref(lua.state, &self.0);
                lua.push_value(lua.state, key.to_lua(lua)?)?;
                lua.push_value(lua.state, value.to_lua(lua)?)?;
                ffi::lua_rawset(lua.state, -3);
                ffi::lua_pop(lua.state, 1);
                Ok(())
            })
        }
    }

    /// Get a field in the table, without invoking metamethods
    pub fn raw_get<K: ToLua<'lua>, V: FromLua<'lua>>(&self, key: K) -> LuaResult<V> {
        let lua = self.0.lua;
        unsafe {
            stack_guard(lua.state, 0, || {
                check_stack(lua.state, 2)?;
                lua.push_ref(lua.state, &self.0);
                lua.push_value(lua.state, key.to_lua(lua)?)?;
                ffi::lua_gettable(lua.state, -2);
                let res = V::from_lua(lua.pop_value(lua.state)?, lua)?;
                ffi::lua_pop(lua.state, 1);
                Ok(res)
            })
        }
    }

    /// Equivalent to the result of the lua '#' operator.
    pub fn length(&self) -> LuaResult<LuaInteger> {
        let lua = self.0.lua;
        unsafe {
            error_guard(lua.state, 0, 0, |state| {
                check_stack(state, 1)?;
                lua.push_ref(state, &self.0);
                Ok(ffi::luaL_len(state, -1))
            })
        }
    }

    /// Equivalent to the result of the lua '#' operator, without invoking the
    /// __len metamethod.
    pub fn raw_length(&self) -> LuaResult<LuaInteger> {
        let lua = self.0.lua;
        unsafe {
            stack_guard(lua.state, 0, || {
                check_stack(lua.state, 1)?;
                lua.push_ref(lua.state, &self.0);
                let len = ffi::lua_rawlen(lua.state, -1);
                ffi::lua_pop(lua.state, 1);
                Ok(len as LuaInteger)
            })
        }
    }

    /// Loop over each key, value pair in the table
    pub fn for_each_pair<K, V, F>(&self, mut f: F) -> LuaResult<()>
        where K: FromLua<'lua>,
              V: FromLua<'lua>,
              F: FnMut(K, V)
    {
        let lua = self.0.lua;
        unsafe {
            stack_guard(lua.state, 0, || {
                check_stack(lua.state, 4)?;
                lua.push_ref(lua.state, &self.0);
                ffi::lua_pushnil(lua.state);

                while ffi::lua_next(lua.state, -2) != 0 {
                    ffi::lua_pushvalue(lua.state, -2);
                    let key = K::from_lua(lua.pop_value(lua.state)?, lua)?;
                    let value = V::from_lua(lua.pop_value(lua.state)?, lua)?;
                    f(key, value);
                }

                ffi::lua_pop(lua.state, 1);
                Ok(())
            })
        }
    }

    /// Loop over the table, strictly interpreting the table as an array, and
    /// fail if it is not a proper lua array.
    pub fn for_each_array_value<V: FromLua<'lua>, F: FnMut(V)>(&self, mut f: F) -> LuaResult<()> {
        let lua = self.0.lua;
        unsafe {
            stack_guard(lua.state, 0, || {
                let mut count = 0;

                check_stack(lua.state, 4)?;
                lua.push_ref(lua.state, &self.0);

                let len = ffi::lua_rawlen(lua.state, -1) as ffi::lua_Integer;
                ffi::lua_pushnil(lua.state);

                while ffi::lua_next(lua.state, -2) != 0 {
                    let mut isnum = 0;
                    let i = ffi::lua_tointegerx(lua.state, -2, &mut isnum);
                    if isnum == 0 {
                        return Err("Not all table keys are integers".into());
                    } else if i > len {
                        return Err("integer key in table is greater than length".into());
                    } else if i <= 0 {
                        return Err("integer key in table is less than 1".into());
                    }

                    // Skip missing keys
                    while count < (i - 1) as usize {
                        f(V::from_lua(LuaNil, lua)?);
                        count += 1;
                    }
                    f(V::from_lua(lua.pop_value(lua.state)?, lua)?);
                    count += 1;
                }

                ffi::lua_pop(lua.state, 1);
                Ok(())
            })
        }
    }

    /// Collect all the pairs in the table into a Vec
    pub fn pairs<K: FromLua<'lua>, V: FromLua<'lua>>(&self) -> LuaResult<Vec<(K, V)>> {
        let mut pairs = Vec::new();
        self.for_each_pair(|k, v| pairs.push((k, v)))?;
        Ok(pairs)
    }

    /// Collect all the values in an array-like table into a Vec
    pub fn array_values<V: FromLua<'lua>>(&self) -> LuaResult<Vec<V>> {
        let mut values = Vec::new();
        self.for_each_array_value(|v| values.push(v))?;
        Ok(values)
    }
}

/// Handle to an an internal lua function
#[derive(Clone, Debug)]
pub struct LuaFunction<'lua>(LuaRef<'lua>);

impl<'lua> LuaFunction<'lua> {
    pub fn call<A: ToLuaMulti<'lua>, R: FromLuaMulti<'lua>>(&self, args: A) -> LuaResult<R> {
        let lua = self.0.lua;
        unsafe {
            stack_guard(lua.state, 0, || {
                let args = args.to_lua_multi(lua)?;
                let nargs = args.len() as c_int;
                check_stack(lua.state, nargs + 3)?;

                let stack_start = ffi::lua_gettop(lua.state);
                lua.push_ref(lua.state, &self.0);
                for arg in args {
                    lua.push_value(lua.state, arg)?;
                }
                handle_error(lua.state,
                             pcall_with_traceback(lua.state, nargs, ffi::LUA_MULTRET))?;
                let nresults = ffi::lua_gettop(lua.state) - stack_start;
                let mut results = LuaMultiValue::new();
                for _ in 0..nresults {
                    results.push_front(lua.pop_value(lua.state)?);
                }
                R::from_lua_multi(results, lua)
            })
        }
    }

    pub fn bind<A: ToLuaMulti<'lua>>(&self, args: A) -> LuaResult<LuaFunction<'lua>> {
        unsafe extern "C" fn bind_call_impl(state: *mut ffi::lua_State) -> c_int {
            let nargs = ffi::lua_gettop(state);

            let nbinds = ffi::lua_tointeger(state, ffi::lua_upvalueindex(2)) as c_int;
            check_stack(state, nbinds + 1).expect("not enough space to handle bound arguments");

            ffi::lua_pushvalue(state, ffi::lua_upvalueindex(1));
            ffi::lua_insert(state, 1);

            // TODO: This is quadratic
            for i in 0..nbinds {
                ffi::lua_pushvalue(state, ffi::lua_upvalueindex(i + 3));
                ffi::lua_insert(state, i + 2);
            }

            ffi::lua_call(state, nargs + nbinds, ffi::LUA_MULTRET);
            ffi::lua_gettop(state)
        }

        let lua = self.0.lua;
        unsafe {
            stack_guard(lua.state, 0, || {
                let args = args.to_lua_multi(lua)?;
                let nargs = args.len() as c_int;

                check_stack(lua.state, nargs + 2)?;
                lua.push_ref(lua.state, &self.0);
                ffi::lua_pushinteger(lua.state, nargs as ffi::lua_Integer);
                for arg in args {
                    lua.push_value(lua.state, arg)?;
                }

                ffi::lua_pushcclosure(lua.state, bind_call_impl, nargs + 2);

                Ok(LuaFunction(lua.pop_ref(lua.state)))
            })
        }
    }
}

/// A LuaThread is Active before the coroutine function finishes, Dead after it finishes, and in
/// Error state if error has been called inside the coroutine.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum LuaThreadStatus {
    Dead,
    Active,
    Error,
}

/// Handle to an an internal lua coroutine
#[derive(Clone, Debug)]
pub struct LuaThread<'lua>(LuaRef<'lua>);

impl<'lua> LuaThread<'lua> {
    /// If this thread has yielded a value, will return Some, otherwise the thread is finished and
    /// this will return None.
    pub fn resume<A: ToLuaMulti<'lua>, R: FromLuaMulti<'lua>>(&self,
                                                              args: A)
                                                              -> LuaResult<Option<R>> {
        let lua = self.0.lua;
        unsafe {
            stack_guard(lua.state, 0, || {
                check_stack(lua.state, 1)?;

                lua.push_ref(lua.state, &self.0);
                let thread_state = ffi::lua_tothread(lua.state, -1);
                ffi::lua_pop(lua.state, 1);

                let args = args.to_lua_multi(lua)?;
                let nargs = args.len() as c_int;
                check_stack(thread_state, nargs)?;

                for arg in args {
                    lua.push_value(thread_state, arg)?;
                }

                handle_error(lua.state,
                             resume_with_traceback(thread_state, lua.state, nargs))?;

                let nresults = ffi::lua_gettop(thread_state);
                let mut results = LuaMultiValue::new();
                for _ in 0..nresults {
                    results.push_front(lua.pop_value(thread_state)?);
                }
                R::from_lua_multi(results, lua).map(|r| Some(r))
            })
        }
    }

    pub fn status(&self) -> LuaResult<LuaThreadStatus> {
        let lua = self.0.lua;
        unsafe {
            stack_guard(lua.state, 0, || {
                check_stack(lua.state, 1)?;

                lua.push_ref(lua.state, &self.0);
                let thread_state = ffi::lua_tothread(lua.state, -1);
                ffi::lua_pop(lua.state, 1);

                let status = ffi::lua_status(thread_state);
                if status != ffi::LUA_OK && status != ffi::LUA_YIELD {
                    Ok(LuaThreadStatus::Error)
                } else if status == ffi::LUA_YIELD || ffi::lua_gettop(thread_state) > 0 {
                    Ok(LuaThreadStatus::Active)
                } else {
                    Ok(LuaThreadStatus::Dead)
                }
            })
        }
    }
}

/// These are the metamethods that can be overridden using this API
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum LuaMetaMethod {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Unm,
    Concat,
    Len,
    Eq,
    Lt,
    Le,
    Index,
    NewIndex,
    Call,
}

/// Methods added will be added to the __index table on the metatable for the userdata, so they can
/// be called as userdata:method(args) as expected.  If there are any regular methods, and an
/// "Index" metamethod is given, it will be called as a *fallback* if the index doesn't match an
/// existing regular method.
pub struct LuaUserDataMethods<T> {
    methods: HashMap<String, LuaCallback>,
    meta_methods: HashMap<LuaMetaMethod, LuaCallback>,
    _type: PhantomData<T>,
}

impl<T: LuaUserDataType> LuaUserDataMethods<T> {
    /// Add a regular method as a function which accepts a &T parameter
    pub fn add_method<M>(&mut self, name: &str, method: M)
        where M: 'static + for<'a, 'lua> FnMut(&'lua Lua, &'a T, LuaMultiValue<'lua>)
                                     -> LuaResult<LuaMultiValue<'lua>>
    {
        self.methods
            .insert(name.to_owned(), Self::box_method(method));
    }

    /// Add a regular method as a function which accepts a &mut T parameter
    pub fn add_method_mut<M>(&mut self, name: &str, method: M)
        where M: 'static + for<'a, 'lua> FnMut(&'lua Lua, &'a mut T, LuaMultiValue<'lua>)
                                     -> LuaResult<LuaMultiValue<'lua>>
    {
        self.methods
            .insert(name.to_owned(), Self::box_method_mut(method));
    }

    /// Add a regular method as a function which accepts generic arguments, the first argument will
    /// always be a LuaUserData of real type T
    pub fn add_function<F>(&mut self, name: &str, function: F)
        where F: 'static + for<'a, 'lua> FnMut(&'lua Lua, LuaMultiValue<'lua>)
                                     -> LuaResult<LuaMultiValue<'lua>>
    {
        self.methods.insert(name.to_owned(), Box::new(function));
    }

    /// Add a metamethod as a function which accepts a &T parameter
    pub fn add_meta_method<M>(&mut self, meta: LuaMetaMethod, method: M)
        where M: 'static + for<'a, 'lua> FnMut(&'lua Lua, &'a T, LuaMultiValue<'lua>)
                                     -> LuaResult<LuaMultiValue<'lua>>
    {
        self.meta_methods.insert(meta, Self::box_method(method));
    }

    /// Add a metamethod as a function which accepts a &mut T parameter
    pub fn add_meta_method_mut<M>(&mut self, meta: LuaMetaMethod, method: M)
        where M: 'static + for<'a, 'lua> FnMut(&'lua Lua, &'a mut T, LuaMultiValue<'lua>)
                                     -> LuaResult<LuaMultiValue<'lua>>
    {
        self.meta_methods.insert(meta, Self::box_method_mut(method));
    }

    /// Add a metamethod as a function which accepts generic arguments, the first argument will
    /// always be a LuaUserData of real type T
    pub fn add_meta_function<F>(&mut self, meta: LuaMetaMethod, function: F)
        where F: 'static + for<'a, 'lua> FnMut(&'lua Lua, LuaMultiValue<'lua>)
                                     -> LuaResult<LuaMultiValue<'lua>>
    {
        self.meta_methods.insert(meta, Box::new(function));
    }

    fn box_method<M>(mut method: M) -> LuaCallback
        where M: 'static + for<'a, 'lua> FnMut(&'lua Lua, &'a T, LuaMultiValue<'lua>)
                                     -> LuaResult<LuaMultiValue<'lua>>
    {
        Box::new(move |lua, mut args| if let Some(front) = args.pop_front() {
                     let userdata = LuaUserData::from_lua(front, lua)?;
                     let userdata = userdata.borrow::<T>()?;
                     method(lua, &userdata, args)
                 } else {
                     Err("No userdata supplied as first argument to method".into())
                 })

    }

    fn box_method_mut<M>(mut method: M) -> LuaCallback
        where M: 'static + for<'a, 'lua> FnMut(&'lua Lua, &'a mut T, LuaMultiValue<'lua>)
                                     -> LuaResult<LuaMultiValue<'lua>>
    {
        Box::new(move |lua, mut args| if let Some(front) = args.pop_front() {
                     let userdata = LuaUserData::from_lua(front, lua)?;
                     let mut userdata = userdata.borrow_mut::<T>()?;
                     method(lua, &mut userdata, args)
                 } else {
                     Err("No userdata supplied as first argument to method".into())
                 })

    }
}

/// Trait for types that can be converted to `LuaUserData`
pub trait LuaUserDataType: 'static + Sized {
    fn add_methods(_methods: &mut LuaUserDataMethods<Self>) {}
}

/// Handle to an internal instance of custom userdata.  All userdata in this API is based around
/// RefCell, to best match the mutable semantics of the lua language.
#[derive(Clone, Debug)]
pub struct LuaUserData<'lua>(LuaRef<'lua>);

impl<'lua> LuaUserData<'lua> {
    pub fn is<T: LuaUserDataType>(&self) -> bool {
        self.inspect(|_: &RefCell<T>| Ok(())).is_ok()
    }

    /// Borrow this userdata out of the internal RefCell that is held in lua.
    pub fn borrow<T: LuaUserDataType>(&self) -> LuaResult<Ref<T>> {
        self.inspect(|cell| Ok(cell.try_borrow()?))
    }

    /// Borrow mutably this userdata out of the internal RefCell that is held in lua.
    pub fn borrow_mut<T: LuaUserDataType>(&self) -> LuaResult<RefMut<T>> {
        self.inspect(|cell| Ok(cell.try_borrow_mut()?))
    }

    fn inspect<'a, T, R, F>(&'a self, func: F) -> LuaResult<R>
        where T: LuaUserDataType,
              F: FnOnce(&'a RefCell<T>) -> LuaResult<R>
    {
        unsafe {
            let lua = self.0.lua;
            stack_guard(lua.state, 0, move || {
                check_stack(lua.state, 3)?;

                lua.push_ref(lua.state, &self.0);
                let userdata = ffi::lua_touserdata(lua.state, -1);
                if userdata.is_null() {
                    return Err("value not userdata".into());
                }

                if ffi::lua_getmetatable(lua.state, -1) == 0 {
                    return Err("value has no metatable".into());
                }

                ffi::lua_rawgeti(lua.state,
                                 ffi::LUA_REGISTRYINDEX,
                                 lua.userdata_metatable::<T>()? as ffi::lua_Integer);
                if ffi::lua_rawequal(lua.state, -1, -2) == 0 {
                    return Err("wrong metatable type for lua userdata".into());
                }

                let res = func(&*(userdata as *const RefCell<T>));

                ffi::lua_pop(lua.state, 3);
                res
            })
        }
    }
}

/// Top level Lua struct which holds the lua state itself.
pub struct Lua {
    state: *mut ffi::lua_State,
    top_state: *mut ffi::lua_State,
    ephemeral: bool,
}

impl Drop for Lua {
    fn drop(&mut self) {
        unsafe {
            if !self.ephemeral {
                ffi::lua_close(self.top_state);
            }
        }
    }
}

impl Lua {
    pub fn new() -> Lua {
        unsafe {
            let state = ffi::luaL_newstate();
            unsafe extern "C" fn panic_function(state: *mut ffi::lua_State) -> c_int {
                if let Some(s) = ffi::lua_tostring(state, -1).as_ref() {
                    panic!("rlua - unprotected error in call to Lua API ({})", s)
                } else {
                    panic!("rlua - unprotected error in call to Lua API <unprintable error>")
                }
            }

            ffi::lua_atpanic(state, panic_function);
            ffi::luaL_openlibs(state);

            stack_guard(state, 0, || {
                ffi::lua_pushlightuserdata(state,
                                           &LUA_USERDATA_REGISTRY_KEY as *const u8 as *mut c_void);

                let registered_userdata =
                    ffi::lua_newuserdata(state,
                                         mem::size_of::<RefCell<HashMap<TypeId, c_int>>>()) as
                    *mut RefCell<HashMap<TypeId, c_int>>;
                ptr::write(registered_userdata, RefCell::new(HashMap::new()));

                ffi::lua_newtable(state);

                push_string(state, "__gc");
                ffi::lua_pushcfunction(state, destructor::<RefCell<HashMap<TypeId, c_int>>>);
                ffi::lua_rawset(state, -3);

                ffi::lua_setmetatable(state, -2);

                ffi::lua_rawset(state, ffi::LUA_REGISTRYINDEX);
                Ok(())
            })
                .unwrap();

            stack_guard(state, 0, || {
                ffi::lua_pushlightuserdata(state,
                                           &FUNCTION_METATABLE_REGISTRY_KEY as *const u8 as
                                           *mut c_void);

                ffi::lua_newtable(state);

                push_string(state, "__gc");
                ffi::lua_pushcfunction(state, destructor::<LuaCallback>);
                ffi::lua_rawset(state, -3);

                push_string(state, "__metatable");
                ffi::lua_pushboolean(state, 0);
                ffi::lua_rawset(state, -3);

                ffi::lua_rawset(state, ffi::LUA_REGISTRYINDEX);
                Ok(())
            })
                .unwrap();

            stack_guard(state, 0, || {
                ffi::lua_rawgeti(state, ffi::LUA_REGISTRYINDEX, ffi::LUA_RIDX_GLOBALS);

                push_string(state, "pcall");
                ffi::lua_pushcfunction(state, safe_pcall);
                ffi::lua_rawset(state, -3);

                push_string(state, "xpcall");
                ffi::lua_pushcfunction(state, safe_xpcall);
                ffi::lua_rawset(state, -3);

                ffi::lua_pop(state, 1);
                Ok(())
            })
                .unwrap();

            stack_guard(state, 0, || {
                ffi::lua_pushlightuserdata(state,
                                           &TOP_STATE_REGISTRY_KEY as *const u8 as *mut c_void);
                ffi::lua_pushlightuserdata(state, state as *mut c_void);
                ffi::lua_rawset(state, ffi::LUA_REGISTRYINDEX);
                Ok(())
            })
                .unwrap();

            Lua {
                state,
                top_state: state,
                ephemeral: false,
            }
        }
    }

    pub fn load(&self, source: &str, name: Option<&str>) -> LuaResult<()> {
        unsafe {
            stack_guard(self.state, 0, || {
                handle_error(self.state,
                             if let Some(name) = name {
                                 let name = CString::new(name.to_owned())?;
                                 ffi::luaL_loadbuffer(self.state,
                                                      source.as_ptr() as *const c_char,
                                                      source.len(),
                                                      name.as_ptr())
                             } else {
                                 ffi::luaL_loadbuffer(self.state,
                                                      source.as_ptr() as *const c_char,
                                                      source.len(),
                                                      ptr::null())
                             })?;

                check_stack(self.state, 2)?;
                handle_error(self.state, pcall_with_traceback(self.state, 0, 0))
            })
        }
    }

    /// Evaluate the given expression or statement inside this Lua state, and if it is an
    /// expression or a statement with return, this returns the value.
    pub fn eval<'lua, R: FromLuaMulti<'lua>>(&'lua self, source: &str) -> LuaResult<R> {
        unsafe {
            stack_guard(self.state, 0, || {
                let stack_start = ffi::lua_gettop(self.state);
                // First, try interpreting the lua as an expression by adding "return", then
                // as a statement.  This is the same thing the actual lua repl does.
                let return_source = "return ".to_owned() + source;
                let mut res = ffi::luaL_loadbuffer(self.state,
                                                   return_source.as_ptr() as *const c_char,
                                                   return_source.len(),
                                                   ptr::null());
                if res == ffi::LUA_ERRSYNTAX {
                    ffi::lua_pop(self.state, 1);
                    res = ffi::luaL_loadbuffer(self.state,
                                               source.as_ptr() as *const c_char,
                                               source.len(),
                                               ptr::null());
                }

                handle_error(self.state, res)?;

                check_stack(self.state, 2)?;
                handle_error(self.state,
                             pcall_with_traceback(self.state, 0, ffi::LUA_MULTRET))?;

                let nresults = ffi::lua_gettop(self.state) - stack_start;
                let mut results = LuaMultiValue::new();
                for _ in 0..nresults {
                    results.push_front(self.pop_value(self.state)?);
                }
                R::from_lua_multi(results, self)
            })
        }
    }

    pub fn create_string(&self, s: &str) -> LuaResult<LuaString> {
        unsafe {
            stack_guard(self.state, 0, || {
                check_stack(self.state, 1)?;
                ffi::lua_pushlstring(self.state, s.as_ptr() as *const c_char, s.len());
                Ok(LuaString(self.pop_ref(self.state)))
            })
        }
    }

    pub fn create_empty_table(&self) -> LuaResult<LuaTable> {
        unsafe {
            stack_guard(self.state, 0, || {
                check_stack(self.state, 1)?;
                ffi::lua_newtable(self.state);
                Ok(LuaTable(self.pop_ref(self.state)))
            })
        }
    }

    pub fn create_table<'lua, K, V, I>(&'lua self, cont: I) -> LuaResult<LuaTable>
        where K: ToLua<'lua>,
              V: ToLua<'lua>,
              I: IntoIterator<Item = (K, V)>
    {
        let table = self.create_empty_table()?;
        for (k, v) in cont {
            table.set(k, v)?;
        }
        Ok(table)
    }

    pub fn create_array_table<'lua, T, I>(&'lua self, cont: I) -> LuaResult<LuaTable>
        where T: ToLua<'lua>,
              I: IntoIterator<Item = T>
    {
        let table = self.create_empty_table()?;
        let mut index = 1;
        for elem in cont {
            table.set(index, elem)?;
            index += 1;
        }
        Ok(table)
    }

    pub fn create_function<F>(&self, func: F) -> LuaResult<LuaFunction>
        where F: 'static + for<'a> FnMut(&'a Lua, LuaMultiValue<'a>) -> LuaResult<LuaMultiValue<'a>>
    {
        self.create_callback_function(Box::new(func))
    }

    pub fn create_thread<'lua>(&'lua self, func: LuaFunction<'lua>) -> LuaResult<LuaThread<'lua>> {
        unsafe {
            stack_guard(self.state, 0, move || {
                check_stack(self.state, 1)?;

                let thread_state = ffi::lua_newthread(self.state);
                self.push_ref(thread_state, &func.0);

                Ok(LuaThread(self.pop_ref(self.state)))
            })
        }
    }

    pub fn create_userdata<T>(&self, data: T) -> LuaResult<LuaUserData>
        where T: LuaUserDataType
    {
        unsafe {
            stack_guard(self.state, 0, move || {
                check_stack(self.state, 2)?;

                let data = RefCell::new(data);
                let data_userdata = ffi::lua_newuserdata(self.state,
                                                         mem::size_of::<RefCell<T>>()) as
                                    *mut RefCell<T>;
                ptr::write(data_userdata, data);

                ffi::lua_rawgeti(self.state,
                                 ffi::LUA_REGISTRYINDEX,
                                 self.userdata_metatable::<T>()? as ffi::lua_Integer);

                ffi::lua_setmetatable(self.state, -2);

                Ok(LuaUserData(self.pop_ref(self.state)))
            })
        }
    }

    pub fn set<'lua, K, V>(&'lua self, key: K, value: V) -> LuaResult<()>
        where K: ToLua<'lua>,
              V: ToLua<'lua>
    {
        unsafe {
            stack_guard(self.state, 0, || {
                check_stack(self.state, 3)?;
                ffi::lua_rawgeti(self.state, ffi::LUA_REGISTRYINDEX, ffi::LUA_RIDX_GLOBALS);
                self.push_value(self.state, key.to_lua(self)?)?;
                self.push_value(self.state, value.to_lua(self)?)?;
                ffi::lua_rawset(self.state, -3);
                ffi::lua_pop(self.state, 1);
                Ok(())
            })
        }
    }

    pub fn get<'lua, K, V>(&'lua self, key: K) -> LuaResult<V>
        where K: ToLua<'lua>,
              V: FromLua<'lua>
    {
        unsafe {
            stack_guard(self.state, 0, || {
                check_stack(self.state, 2)?;
                ffi::lua_rawgeti(self.state, ffi::LUA_REGISTRYINDEX, ffi::LUA_RIDX_GLOBALS);
                self.push_value(self.state, key.to_lua(self)?)?;
                ffi::lua_gettable(self.state, -2);
                let ret = self.pop_value(self.state)?;
                ffi::lua_pop(self.state, 1);
                V::from_lua(ret, self)
            })
        }
    }

    pub fn coerce_string<'lua>(&'lua self, v: LuaValue<'lua>) -> LuaResult<LuaString<'lua>> {
        match v {
            LuaValue::String(s) => Ok(s),
            v => unsafe {
                stack_guard(self.state, 0, || {
                    check_stack(self.state, 1)?;
                    self.push_value(self.state, v)?;
                    if ffi::lua_tostring(self.state, -1).is_null() {
                        Err("cannot convert lua value to string".into())
                    } else {
                        Ok(LuaString(self.pop_ref(self.state)))
                    }
                })
            },
        }
    }

    pub fn coerce_integer(&self, v: LuaValue) -> LuaResult<LuaInteger> {
        match v {
            LuaValue::Integer(i) => Ok(i),
            LuaValue::Number(n) => Ok(n as LuaInteger),
            v => unsafe {
                stack_guard(self.state, 0, || {
                    check_stack(self.state, 1)?;
                    self.push_value(self.state, v)?;
                    let mut isint = 0;
                    let i = ffi::lua_tointegerx(self.state, -1, &mut isint);
                    if isint == 0 {
                        Err("cannot convert lua value to integer".into())
                    } else {
                        ffi::lua_pop(self.state, 1);
                        Ok(i)
                    }
                })
            },
        }
    }

    pub fn coerce_number(&self, v: LuaValue) -> LuaResult<LuaNumber> {
        match v {
            LuaValue::Integer(i) => Ok(i as LuaNumber),
            LuaValue::Number(n) => Ok(n),
            v => unsafe {
                stack_guard(self.state, 0, || {
                    check_stack(self.state, 1)?;
                    self.push_value(self.state, v)?;
                    let mut isnum = 0;
                    let n = ffi::lua_tonumberx(self.state, -1, &mut isnum);
                    if isnum == 0 {
                        Err("cannot convert lua value to number".into())
                    } else {
                        ffi::lua_pop(self.state, 1);
                        Ok(n)
                    }
                })
            },
        }
    }

    pub fn from<'lua, T: ToLua<'lua>>(&'lua self, t: T) -> LuaResult<LuaValue<'lua>> {
        t.to_lua(self)
    }

    pub fn to<'lua, T: FromLua<'lua>>(&'lua self, value: LuaValue<'lua>) -> LuaResult<T> {
        T::from_lua(value, self)
    }

    pub fn pack<'lua, T: ToLuaMulti<'lua>>(&'lua self, t: T) -> LuaResult<LuaMultiValue<'lua>> {
        t.to_lua_multi(self)
    }

    pub fn unpack<'lua, T: FromLuaMulti<'lua>>(&'lua self,
                                               value: LuaMultiValue<'lua>)
                                               -> LuaResult<T> {
        T::from_lua_multi(value, self)
    }

    fn create_callback_function(&self, func: LuaCallback) -> LuaResult<LuaFunction> {
        unsafe extern "C" fn callback_call_impl(state: *mut ffi::lua_State) -> c_int {
            callback_error(state, || {
                ffi::lua_pushlightuserdata(state,
                                           &TOP_STATE_REGISTRY_KEY as *const u8 as *mut c_void);
                ffi::lua_gettable(state, ffi::LUA_REGISTRYINDEX);
                let top_state = ffi::lua_touserdata(state, -1) as *mut ffi::lua_State;
                ffi::lua_pop(state, 1);

                let lua = Lua {
                    state: state,
                    top_state: top_state,
                    ephemeral: true,
                };

                let func = &mut *(ffi::lua_touserdata(state, ffi::lua_upvalueindex(1)) as
                                  *mut LuaCallback);

                let nargs = ffi::lua_gettop(state);
                let mut args = LuaMultiValue::new();
                for _ in 0..nargs {
                    args.push_front(lua.pop_value(state)?);
                }

                let results = func(&lua, args)?;
                let nresults = results.len() as c_int;

                for r in results {
                    lua.push_value(state, r)?;
                }

                Ok(nresults)
            })
        }

        unsafe {
            stack_guard(self.state, 0, move || {
                check_stack(self.state, 2)?;

                let func_userdata = ffi::lua_newuserdata(self.state,
                                                         mem::size_of::<LuaCallback>()) as
                                    *mut LuaCallback;
                ptr::write(func_userdata, func);

                ffi::lua_pushlightuserdata(self.state,
                                           &FUNCTION_METATABLE_REGISTRY_KEY as *const u8 as
                                           *mut c_void);
                ffi::lua_gettable(self.state, ffi::LUA_REGISTRYINDEX);
                ffi::lua_setmetatable(self.state, -2);

                ffi::lua_pushcclosure(self.state, callback_call_impl, 1);

                Ok(LuaFunction(self.pop_ref(self.state)))
            })
        }
    }

    unsafe fn push_value(&self, state: *mut ffi::lua_State, value: LuaValue) -> LuaResult<()> {
        stack_guard(state, 1, move || {
            match value {
                LuaValue::Nil => {
                    ffi::lua_pushnil(state);
                }

                LuaValue::Boolean(b) => {
                    ffi::lua_pushboolean(state, if b { 1 } else { 0 });
                }

                LuaValue::LightUserData(ud) => {
                    ffi::lua_pushlightuserdata(state, ud.0);
                }

                LuaValue::Integer(i) => {
                    ffi::lua_pushinteger(state, i);
                }

                LuaValue::Number(n) => {
                    ffi::lua_pushnumber(state, n);
                }

                LuaValue::String(s) => {
                    self.push_ref(state, &s.0);
                }

                LuaValue::Table(t) => {
                    self.push_ref(state, &t.0);
                }

                LuaValue::Function(f) => {
                    self.push_ref(state, &f.0);
                }

                LuaValue::UserData(ud) => {
                    self.push_ref(state, &ud.0);
                }

                LuaValue::Thread(t) => {
                    self.push_ref(state, &t.0);
                }
            }
            Ok(())
        })
    }

    unsafe fn pop_value(&self, state: *mut ffi::lua_State) -> LuaResult<LuaValue> {
        stack_guard(state, -1, || match ffi::lua_type(state, -1) {
            ffi::LUA_TNIL => {
                ffi::lua_pop(state, 1);
                Ok(LuaNil)
            }

            ffi::LUA_TBOOLEAN => {
                let b = LuaValue::Boolean(ffi::lua_toboolean(state, -1) != 0);
                ffi::lua_pop(state, 1);
                Ok(b)
            }

            ffi::LUA_TLIGHTUSERDATA => {
                let ud = LuaValue::LightUserData(LightUserData(ffi::lua_touserdata(state, -1)));
                ffi::lua_pop(state, 1);
                Ok(ud)
            }

            ffi::LUA_TNUMBER => {
                if ffi::lua_isinteger(state, -1) != 0 {
                    let i = LuaValue::Integer(ffi::lua_tointeger(state, -1));
                    ffi::lua_pop(state, 1);
                    Ok(i)
                } else {
                    let n = LuaValue::Number(ffi::lua_tonumber(state, -1));
                    ffi::lua_pop(state, 1);
                    Ok(n)
                }
            }

            ffi::LUA_TSTRING => Ok(LuaValue::String(LuaString(self.pop_ref(state)))),

            ffi::LUA_TTABLE => Ok(LuaValue::Table(LuaTable(self.pop_ref(state)))),

            ffi::LUA_TFUNCTION => Ok(LuaValue::Function(LuaFunction(self.pop_ref(state)))),

            ffi::LUA_TUSERDATA => Ok(LuaValue::UserData(LuaUserData(self.pop_ref(state)))),

            ffi::LUA_TTHREAD => Ok(LuaValue::Thread(LuaThread(self.pop_ref(state)))),

            _ => Err("Unsupported type in pop_value".into()),
        })
    }

    unsafe fn push_ref(&self, state: *mut ffi::lua_State, lref: &LuaRef) {
        assert_eq!(lref.lua.top_state,
                   self.top_state,
                   "Lua instance passed LuaValue created from a different Lua");

        ffi::lua_rawgeti(state,
                         ffi::LUA_REGISTRYINDEX,
                         lref.registry_id as ffi::lua_Integer);
    }

    unsafe fn pop_ref(&self, state: *mut ffi::lua_State) -> LuaRef {
        let registry_id = ffi::luaL_ref(state, ffi::LUA_REGISTRYINDEX);
        LuaRef {
            lua: self,
            registry_id: registry_id,
        }
    }

    unsafe fn userdata_metatable<T: LuaUserDataType>(&self) -> LuaResult<c_int> {
        // Used if both an __index metamethod is set and regular methods, checks methods table
        // first, then __index metamethod.
        unsafe extern "C" fn meta_index_impl(state: *mut ffi::lua_State) -> c_int {
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

        stack_guard(self.state, 0, move || {
            check_stack(self.state, 3)?;

            ffi::lua_pushlightuserdata(self.state,
                                       &LUA_USERDATA_REGISTRY_KEY as *const u8 as *mut c_void);
            ffi::lua_gettable(self.state, ffi::LUA_REGISTRYINDEX);
            let registered_userdata = ffi::lua_touserdata(self.state, -1) as
                                      *mut RefCell<HashMap<TypeId, c_int>>;
            let mut map = (*registered_userdata).borrow_mut();
            ffi::lua_pop(self.state, 1);

            match map.entry(TypeId::of::<T>()) {
                HashMapEntry::Occupied(entry) => Ok(*entry.get()),
                HashMapEntry::Vacant(entry) => {
                    ffi::lua_newtable(self.state);

                    let mut methods = LuaUserDataMethods {
                        methods: HashMap::new(),
                        meta_methods: HashMap::new(),
                        _type: PhantomData,
                    };
                    T::add_methods(&mut methods);

                    let has_methods = !methods.methods.is_empty();

                    if has_methods {
                        push_string(self.state, "__index");
                        ffi::lua_newtable(self.state);

                        check_stack(self.state, methods.methods.len() as c_int * 2)?;
                        for (k, m) in methods.methods {
                            push_string(self.state, &k);
                            self.push_value(self.state,
                                            LuaValue::Function(self.create_callback_function(m)?))?;
                            ffi::lua_rawset(self.state, -3);
                        }

                        ffi::lua_rawset(self.state, -3);
                    }

                    check_stack(self.state, methods.meta_methods.len() as c_int * 2)?;
                    for (k, m) in methods.meta_methods {
                        if k == LuaMetaMethod::Index && has_methods {
                            push_string(self.state, "__index");
                            ffi::lua_pushvalue(self.state, -1);
                            ffi::lua_gettable(self.state, -3);
                            self.push_value(self.state,
                                            LuaValue::Function(self.create_callback_function(m)?))?;
                            ffi::lua_pushcclosure(self.state, meta_index_impl, 2);
                            ffi::lua_rawset(self.state, -3);
                        } else {
                            let name = match k {
                                LuaMetaMethod::Add => "__add",
                                LuaMetaMethod::Sub => "__sub",
                                LuaMetaMethod::Mul => "__mul",
                                LuaMetaMethod::Div => "__div",
                                LuaMetaMethod::Mod => "__mod",
                                LuaMetaMethod::Pow => "__pow",
                                LuaMetaMethod::Unm => "__unm",
                                LuaMetaMethod::Concat => "__concat",
                                LuaMetaMethod::Len => "__len",
                                LuaMetaMethod::Eq => "__eq",
                                LuaMetaMethod::Lt => "__lt",
                                LuaMetaMethod::Le => "__le",
                                LuaMetaMethod::Index => "__index",
                                LuaMetaMethod::NewIndex => "__newIndex",
                                LuaMetaMethod::Call => "__call",
                            };
                            push_string(self.state, name);
                            self.push_value(self.state,
                                            LuaValue::Function(self.create_callback_function(m)?))?;
                            ffi::lua_rawset(self.state, -3);
                        }
                    }

                    push_string(self.state, "__gc");
                    ffi::lua_pushcfunction(self.state, destructor::<RefCell<T>>);
                    ffi::lua_rawset(self.state, -3);

                    push_string(self.state, "__metatable");
                    ffi::lua_pushboolean(self.state, 0);
                    ffi::lua_rawset(self.state, -3);

                    let id = ffi::luaL_ref(self.state, ffi::LUA_REGISTRYINDEX);
                    entry.insert(id);
                    Ok(id)
                }
            }
        })
    }
}

static LUA_USERDATA_REGISTRY_KEY: u8 = 0;
static FUNCTION_METATABLE_REGISTRY_KEY: u8 = 0;
static TOP_STATE_REGISTRY_KEY: u8 = 0;
