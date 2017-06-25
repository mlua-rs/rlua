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

/// A dynamically typed Lua value.
#[derive(Debug, Clone)]
pub enum LuaValue<'lua> {
    /// The Lua value `nil`.
    Nil,
    /// The Lua value `true` or `false`.
    Boolean(bool),
    /// A "light userdata" object, equivalent to a raw pointer.
    LightUserData(LightUserData),
    /// An integer number.
    ///
    /// Any Lua number convertible to a `LuaInteger` will be represented as this variant.
    Integer(LuaInteger),
    /// A floating point number.
    Number(LuaNumber),
    /// An interned string, managed by Lua.
    ///
    /// Unlike Rust strings, Lua strings may not be valid UTF-8.
    String(LuaString<'lua>),
    /// Reference to a Lua table.
    Table(LuaTable<'lua>),
    /// Reference to a Lua function (or closure).
    Function(LuaFunction<'lua>),
    /// Reference to a "full" userdata object.
    UserData(LuaUserData<'lua>),
    /// Reference to a Lua thread (or coroutine).
    Thread(LuaThread<'lua>),
}
pub use self::LuaValue::Nil as LuaNil;

/// Trait for types convertible to `LuaValue`.
pub trait ToLua<'a> {
    /// Performs the conversion.
    fn to_lua(self, lua: &'a Lua) -> LuaResult<LuaValue<'a>>;
}

/// Trait for types convertible from `LuaValue`.
pub trait FromLua<'a>: Sized {
    /// Performs the conversion.
    fn from_lua(lua_value: LuaValue<'a>, lua: &'a Lua) -> LuaResult<Self>;
}

/// Multiple Lua values used for both argument passing and also for multiple return values.
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

/// Trait for types convertible to any number of Lua values.
///
/// This is a generalization of `ToLua`, allowing any number of resulting Lua
/// values instead of just one. Any type that implements `ToLua` will
/// automatically implement this trait.
pub trait ToLuaMulti<'a> {
    /// Performs the conversion.
    fn to_lua_multi(self, lua: &'a Lua) -> LuaResult<LuaMultiValue<'a>>;
}

/// Trait for types that can be created from an arbitrary number of Lua values.
///
/// This is a generalization of `FromLua`, allowing an arbitrary number of Lua
/// values to participate in the conversion. Any type that implements `FromLua`
/// will automatically implement this trait.
pub trait FromLuaMulti<'a>: Sized {
    /// Performs the conversion.
    ///
    /// In case `values` contains more values than needed to perform the
    /// conversion, the excess values should be ignored. This reflects the
    /// semantics of Lua when calling a function or assigning values. Similarly,
    /// if not enough values are given, conversions should assume that any
    /// missing values are nil.
    fn from_lua_multi(values: LuaMultiValue<'a>, lua: &'a Lua) -> LuaResult<Self>;
}

impl<'lua> ToLua<'lua> for LuaError {
    fn to_lua(self, lua: &'lua Lua) -> LuaResult<LuaValue<'lua>> {
        unsafe {
            push_wrapped_error(lua.state, self);
            Ok(lua.pop_value(lua.state))
        }
    }
}

type LuaCallback = Box<
    for<'lua> FnMut(&'lua Lua, LuaMultiValue<'lua>)
                    -> LuaResult<LuaMultiValue<'lua>>,
>;

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

/// Type of Lua integer numbers.
pub type LuaInteger = ffi::lua_Integer;
/// Type of Lua floating point numbers.
pub type LuaNumber = ffi::lua_Number;

/// A "light" userdata value. Equivalent to an unmanaged raw pointer.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct LightUserData(pub *mut c_void);

/// Handle to an internal Lua string.
///
/// Unlike Rust strings, Lua strings may not be valid UTF-8.
#[derive(Clone, Debug)]
pub struct LuaString<'lua>(LuaRef<'lua>);

impl<'lua> LuaString<'lua> {
    /// Get a `&str` slice if the Lua string is valid UTF-8.
    ///
    /// # Example
    ///
    /// ```
    /// # extern crate rlua;
    /// # use rlua::*;
    /// # fn main() {
    /// let lua = Lua::new();
    /// let globals = lua.globals().unwrap();
    ///
    /// let version: LuaString = globals.get("_VERSION").unwrap();
    /// assert!(version.to_str().unwrap().contains("Lua"));
    ///
    /// let non_utf8: LuaString = lua.eval(r#"  "test\xff"  "#).unwrap();
    /// assert!(non_utf8.to_str().is_err());
    /// # }
    /// ```
    pub fn to_str(&self) -> LuaResult<&str> {
        let lua = self.0.lua;
        unsafe {
            stack_guard(lua.state, 0, || {
                check_stack(lua.state, 1)?;
                lua.push_ref(lua.state, &self.0);
                assert_eq!(ffi::lua_type(lua.state, -1), ffi::LUA_TSTRING);
                let s = CStr::from_ptr(ffi::lua_tostring(lua.state, -1))
                    .to_str()
                    .map_err(|e| LuaConversionError::Utf8Error(e))?;
                ffi::lua_pop(lua.state, 1);
                Ok(s)
            })
        }
    }
}

/// Handle to an internal Lua table.
#[derive(Clone, Debug)]
pub struct LuaTable<'lua>(LuaRef<'lua>);

impl<'lua> LuaTable<'lua> {
    /// Sets a key-value pair in the table.
    ///
    /// If the value is `nil`, this will effectively remove the pair.
    ///
    /// This might invoke the `__newindex` metamethod. Use the `raw_set` method
    /// if that is not desired.
    pub fn set<K: ToLua<'lua>, V: ToLua<'lua>>(&self, key: K, value: V) -> LuaResult<()> {
        let lua = self.0.lua;
        let key = key.to_lua(lua)?;
        let value = value.to_lua(lua)?;
        unsafe {
            error_guard(lua.state, 0, 0, |state| {
                check_stack(state, 3)?;
                lua.push_ref(state, &self.0);
                lua.push_value(state, key);
                lua.push_value(state, value);
                ffi::lua_settable(state, -3);
                Ok(())
            })
        }
    }

    /// Gets the value associated to `key` from the table.
    ///
    /// If no value is associated to `key`, returns the `nil` value.
    ///
    /// This might invoke the `__index` metamethod. Use the `raw_get` method if
    /// that is not desired.
    pub fn get<K: ToLua<'lua>, V: FromLua<'lua>>(&self, key: K) -> LuaResult<V> {
        let lua = self.0.lua;
        let key = key.to_lua(lua)?;
        unsafe {
            let res = error_guard(lua.state, 0, 0, |state| {
                check_stack(state, 2)?;
                lua.push_ref(state, &self.0);
                lua.push_value(state, key.to_lua(lua)?);
                ffi::lua_gettable(state, -2);
                let res = lua.pop_value(state);
                ffi::lua_pop(state, 1);
                Ok(res)
            })?;
            V::from_lua(res, lua)
        }
    }

    /// Checks whether the table contains a non-nil value for `key`.
    pub fn contains_key<K: ToLua<'lua>>(&self, key: K) -> LuaResult<bool> {
        let lua = self.0.lua;
        let key = key.to_lua(lua)?;
        unsafe {
            error_guard(lua.state, 0, 0, |state| {
                check_stack(state, 2)?;
                lua.push_ref(state, &self.0);
                lua.push_value(state, key);
                ffi::lua_gettable(state, -2);
                let has = ffi::lua_isnil(state, -1) == 0;
                ffi::lua_pop(state, 2);
                Ok(has)
            })
        }
    }

    /// Sets a key-value pair without invoking metamethods.
    pub fn raw_set<K: ToLua<'lua>, V: ToLua<'lua>>(&self, key: K, value: V) -> LuaResult<()> {
        let lua = self.0.lua;
        unsafe {
            stack_guard(lua.state, 0, || {
                check_stack(lua.state, 3)?;
                lua.push_ref(lua.state, &self.0);
                lua.push_value(lua.state, key.to_lua(lua)?);
                lua.push_value(lua.state, value.to_lua(lua)?);
                ffi::lua_rawset(lua.state, -3);
                ffi::lua_pop(lua.state, 1);
                Ok(())
            })
        }
    }

    /// Gets the value associated to `key` without invoking metamethods.
    pub fn raw_get<K: ToLua<'lua>, V: FromLua<'lua>>(&self, key: K) -> LuaResult<V> {
        let lua = self.0.lua;
        unsafe {
            stack_guard(lua.state, 0, || {
                check_stack(lua.state, 2)?;
                lua.push_ref(lua.state, &self.0);
                lua.push_value(lua.state, key.to_lua(lua)?);
                ffi::lua_gettable(lua.state, -2);
                let res = V::from_lua(lua.pop_value(lua.state), lua)?;
                ffi::lua_pop(lua.state, 1);
                Ok(res)
            })
        }
    }

    /// Returns the result of the Lua `#` operator.
    ///
    /// This might invoke the `__len` metamethod. Use the `raw_len` method if
    /// that is not desired.
    pub fn len(&self) -> LuaResult<LuaInteger> {
        let lua = self.0.lua;
        unsafe {
            error_guard(lua.state, 0, 0, |state| {
                check_stack(state, 1)?;
                lua.push_ref(state, &self.0);
                Ok(ffi::luaL_len(state, -1))
            })
        }
    }

    /// Returns the result of the Lua `#` operator, without invoking the
    /// `__len` metamethod.
    pub fn raw_len(&self) -> LuaResult<LuaInteger> {
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

    /// Consume this table and return an iterator over the pairs of the table,
    /// works like the Lua 'pairs' function.
    pub fn pairs<K: FromLua<'lua>, V: FromLua<'lua>>(self) -> LuaTablePairs<'lua, K, V> {
        let next_key = Some(LuaRef {
            lua: self.0.lua,
            registry_id: ffi::LUA_REFNIL,
        });

        LuaTablePairs {
            table: self.0,
            next_key,
            _phantom: PhantomData,
        }
    }

    /// Consume this table and return an iterator over the values of this table,
    /// which should be a sequence.  Works like the Lua 'ipairs' function, but
    /// doesn't return the indexes, only the values in order.
    pub fn sequence_values<V: FromLua<'lua>>(self) -> LuaTableSequence<'lua, V> {
        LuaTableSequence {
            table: self.0,
            index: Some(1),
            _phantom: PhantomData,
        }
    }
}

/// An iterator over the pairs of a Lua table.
///
/// Should behave exactly like the lua 'pairs' function.  Holds an internal
/// reference to the table.
pub struct LuaTablePairs<'lua, K, V> {
    table: LuaRef<'lua>,
    next_key: Option<LuaRef<'lua>>,
    _phantom: PhantomData<(K, V)>,
}

impl<'lua, K, V> Iterator for LuaTablePairs<'lua, K, V>
where
    K: FromLua<'lua>,
    V: FromLua<'lua>,
{
    type Item = LuaResult<(K, V)>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(next_key) = self.next_key.take() {
            let lua = self.table.lua;

            unsafe {
                if let Err(e) = check_stack(lua.state, 4) {
                    return Some(Err(e));
                }

                lua.push_ref(lua.state, &self.table);
                lua.push_ref(lua.state, &next_key);

                match error_guard(lua.state, 2, 0, |state| if ffi::lua_next(state, -2) != 0 {
                    ffi::lua_pushvalue(state, -2);
                    let key = lua.pop_value(state);
                    let value = lua.pop_value(state);
                    let next_key = lua.pop_ref(lua.state);
                    ffi::lua_pop(lua.state, 1);
                    Ok(Some((key, value, next_key)))
                } else {
                    ffi::lua_pop(lua.state, 1);
                    Ok(None)
                }) {
                    Ok(Some((key, value, next_key))) => {
                        self.next_key = Some(next_key);
                        Some((|| {
                             let key = K::from_lua(key, lua)?;
                             let value = V::from_lua(value, lua)?;
                             Ok((key, value))
                         })())
                    }
                    Ok(None) => None,
                    Err(e) => Some(Err(e)),
                }
            }
        } else {
            None
        }
    }
}

/// An iterator over the sequence part of a Lua table.
///
/// Should behave similarly to the lua 'ipairs" function, except only produces
/// the values, not the indexes.  Holds an internal reference to the table.
pub struct LuaTableSequence<'lua, V> {
    table: LuaRef<'lua>,
    index: Option<LuaInteger>,
    _phantom: PhantomData<V>,
}

impl<'lua, V> Iterator for LuaTableSequence<'lua, V>
where
    V: FromLua<'lua>,
{
    type Item = LuaResult<V>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(index) = self.index.take() {
            let lua = self.table.lua;

            unsafe {
                if let Err(e) = check_stack(lua.state, 2) {
                    return Some(Err(e));
                }

                lua.push_ref(lua.state, &self.table);
                match error_guard(
                    lua.state,
                    1,
                    0,
                    |state| if ffi::lua_geti(state, -1, index) != ffi::LUA_TNIL {
                        let value = lua.pop_value(state);
                        ffi::lua_pop(state, 1);
                        Ok(Some(value))
                    } else {
                        ffi::lua_pop(state, 2);
                        Ok(None)
                    },
                ) {
                    Ok(Some(r)) => {
                        self.index = Some(index + 1);
                        Some(V::from_lua(r, lua))
                    }
                    Ok(None) => None,
                    Err(e) => Some(Err(e)),
                }
            }
        } else {
            None
        }
    }
}

/// Handle to an internal Lua function.
#[derive(Clone, Debug)]
pub struct LuaFunction<'lua>(LuaRef<'lua>);

impl<'lua> LuaFunction<'lua> {
    /// Calls the function, passing `args` as function arguments.
    ///
    /// The function's return values are converted to the generic type `R`.
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
                    lua.push_value(lua.state, arg);
                }
                handle_error(
                    lua.state,
                    pcall_with_traceback(lua.state, nargs, ffi::LUA_MULTRET),
                )?;
                let nresults = ffi::lua_gettop(lua.state) - stack_start;
                let mut results = LuaMultiValue::new();
                for _ in 0..nresults {
                    results.push_front(lua.pop_value(lua.state));
                }
                R::from_lua_multi(results, lua)
            })
        }
    }

    /// Returns a function that, when called with no arguments, calls `self`, passing `args` as
    /// arguments.
    ///
    /// This is equivalent to this Lua code:
    ///
    /// ```notrust
    /// function bind(f, ...)
    ///     return function() f(...) end
    /// end
    /// ```
    ///
    /// # Example
    ///
    /// ```
    /// # extern crate rlua;
    /// # use rlua::*;
    ///
    /// # fn main() {
    /// let lua = Lua::new();
    /// let globals = lua.globals().unwrap();
    ///
    /// // Bind the argument `123` to Lua's `tostring` function
    /// let tostring: LuaFunction = globals.get("tostring").unwrap();
    /// let tostring_123: LuaFunction = tostring.bind(123i32).unwrap();
    ///
    /// // Now we can call `tostring_123` without arguments to get the result of `tostring(123)`
    /// let result: String = tostring_123.call(()).unwrap();
    /// assert_eq!(result, "123");
    /// # }
    /// ```
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
                    lua.push_value(lua.state, arg);
                }

                ffi::lua_pushcclosure(lua.state, bind_call_impl, nargs + 2);

                Ok(LuaFunction(lua.pop_ref(lua.state)))
            })
        }
    }
}

/// Status of a Lua thread (or coroutine).
///
/// A `LuaThread` is `Active` before the coroutine function finishes, Dead after
/// it finishes, and in Error state if error has been called inside the
/// coroutine.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum LuaThreadStatus {
    /// The thread has finished executing.
    Dead,
    /// The thread is currently running or suspended because it has called `coroutine.yield`.
    Active,
    /// The thread has thrown an error during execution.
    Error,
}

/// Handle to an internal Lua thread (or coroutine).
#[derive(Clone, Debug)]
pub struct LuaThread<'lua>(LuaRef<'lua>);

impl<'lua> LuaThread<'lua> {
    /// Resumes execution of this thread.
    ///
    /// Equivalent to `coroutine.resume`.
    ///
    /// Passes `args` as arguments to the thread. If the coroutine has called `coroutine.yield`, it
    /// will return these arguments. Otherwise, the coroutine wasn't yet started, so the arguments
    /// are passed to its main function.
    ///
    /// If the thread is no longer in `Active` state (meaning it has finished execution or
    /// encountered an error), this will return Err(CoroutineInactive),
    /// otherwise will return Ok as follows:
    ///
    /// If the thread calls `coroutine.yield`, returns the values passed to `yield`. If the thread
    /// `return`s values from its main function, returns those.
    ///
    /// # Example
    ///
    /// ```
    /// # extern crate rlua;
    /// # use rlua::*;
    ///
    /// # fn main() {
    /// let lua = Lua::new();
    /// let thread: LuaThread = lua.eval(r#"
    ///     coroutine.create(function(arg)
    ///         assert(arg == 42)
    ///         local yieldarg = coroutine.yield(123)
    ///         assert(yieldarg == 43)
    ///         return 987
    ///     end)
    /// "#).unwrap();
    ///
    /// assert_eq!(thread.resume::<_, u32>(42).unwrap(), 123);
    /// assert_eq!(thread.resume::<_, u32>(43).unwrap(), 987);
    ///
    /// // The coroutine has now returned, so `resume` will fail
    /// match thread.resume::<_, u32>(()) {
    ///     Err(LuaError::CoroutineInactive) => {},
    ///     unexpected => panic!("unexpected result {:?}", unexpected),
    /// }
    /// # }
    /// ```
    pub fn resume<A, R>(&self, args: A) -> LuaResult<R>
    where
        A: ToLuaMulti<'lua>,
        R: FromLuaMulti<'lua>,
    {
        let lua = self.0.lua;
        unsafe {
            stack_guard(lua.state, 0, || {
                check_stack(lua.state, 1)?;

                lua.push_ref(lua.state, &self.0);
                let thread_state = ffi::lua_tothread(lua.state, -1);

                let status = ffi::lua_status(thread_state);
                if status != ffi::LUA_YIELD && ffi::lua_gettop(thread_state) == 0 {
                    return Err(LuaError::CoroutineInactive);
                }

                ffi::lua_pop(lua.state, 1);

                let args = args.to_lua_multi(lua)?;
                let nargs = args.len() as c_int;
                check_stack(thread_state, nargs)?;

                for arg in args {
                    lua.push_value(thread_state, arg);
                }

                handle_error(
                    lua.state,
                    resume_with_traceback(thread_state, lua.state, nargs),
                )?;

                let nresults = ffi::lua_gettop(thread_state);
                let mut results = LuaMultiValue::new();
                for _ in 0..nresults {
                    results.push_front(lua.pop_value(thread_state));
                }
                R::from_lua_multi(results, lua)
            })
        }
    }

    /// Gets the status of the thread.
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

/// Kinds of metamethods that can be overridden.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum LuaMetaMethod {
    /// The `+` operator.
    Add,
    /// The `-` operator.
    Sub,
    /// The `*` operator.
    Mul,
    /// The `/` operator.
    Div,
    /// The `%` operator.
    Mod,
    /// The `^` operator.
    Pow,
    /// The unary minus (`-`) operator.
    Unm,
    /// The floor division (//) operator.
    IDiv,
    /// The bitwise AND (&) operator.
    BAnd,
    /// The bitwise OR (|) operator.
    BOr,
    /// The bitwise XOR (binary ~) operator.
    BXor,
    /// The bitwise NOT (unary ~) operator.
    BNot,
    /// The bitwise left shift (<<) operator.
    Shl,
    /// The bitwise right shift (>>) operator.
    Shr,
    /// The string concatenation operator `..`.
    Concat,
    /// The length operator `#`.
    Len,
    /// The `==` operator.
    Eq,
    /// The `<` operator.
    Lt,
    /// The `<=` operator.
    Le,
    /// Index access `obj[key]`.
    Index,
    /// Index write access `obj[key] = value`.
    NewIndex,
    /// The call "operator" `obj(arg1, args2, ...)`.
    Call,
    /// tostring(ud) will call this if it exists
    ToString,
}

/// Stores methods of a userdata object.
///
/// Methods added will be added to the `__index` table on the metatable for the
/// userdata, so they can be called as `userdata:method(args)` as expected.  If
/// there are any regular methods, and an `Index` metamethod is given, it will
/// be called as a *fallback* if the index doesn't match an existing regular
/// method.
pub struct LuaUserDataMethods<T> {
    methods: HashMap<String, LuaCallback>,
    meta_methods: HashMap<LuaMetaMethod, LuaCallback>,
    _type: PhantomData<T>,
}

impl<T: LuaUserDataType> LuaUserDataMethods<T> {
    /// Add a regular method as a function which accepts a &T as the first
    /// parameter.
    pub fn add_method<M>(&mut self, name: &str, method: M)
        where M: 'static + for<'a, 'lua> FnMut(&'lua Lua, &'a T, LuaMultiValue<'lua>)
                                     -> LuaResult<LuaMultiValue<'lua>>
    {
        self.methods.insert(
            name.to_owned(),
            Self::box_method(method),
        );
    }

    /// Add a regular method as a function which accepts a &mut T as the first
    /// parameter.
    pub fn add_method_mut<M>(&mut self, name: &str, method: M)
        where M: 'static + for<'a, 'lua> FnMut(&'lua Lua, &'a mut T, LuaMultiValue<'lua>)
                                     -> LuaResult<LuaMultiValue<'lua>>
    {
        self.methods.insert(
            name.to_owned(),
            Self::box_method_mut(method),
        );
    }

    /// Add a regular method as a function which accepts generic arguments, the
    /// first argument will always be a LuaUserData of type T.
    pub fn add_function<F>(&mut self, name: &str, function: F)
        where F: 'static + for<'a, 'lua> FnMut(&'lua Lua, LuaMultiValue<'lua>)
                                     -> LuaResult<LuaMultiValue<'lua>>
    {
        self.methods.insert(name.to_owned(), Box::new(function));
    }

    /// Add a metamethod as a function which accepts a &T as the first
    /// parameter.  This can cause an error with certain binary metamethods that
    /// can trigger if ony the right side has a metatable.
    pub fn add_meta_method<M>(&mut self, meta: LuaMetaMethod, method: M)
        where M: 'static + for<'a, 'lua> FnMut(&'lua Lua, &'a T, LuaMultiValue<'lua>)
                                     -> LuaResult<LuaMultiValue<'lua>>
    {
        self.meta_methods.insert(meta, Self::box_method(method));
    }

    /// Add a metamethod as a function which accepts a &mut T as the first
    /// parameter.  This can cause an error with certain binary metamethods that
    /// can trigger if ony the right side has a metatable.
    pub fn add_meta_method_mut<M>(&mut self, meta: LuaMetaMethod, method: M)
        where M: 'static + for<'a, 'lua> FnMut(&'lua Lua, &'a mut T, LuaMultiValue<'lua>)
                                     -> LuaResult<LuaMultiValue<'lua>>
    {
        self.meta_methods.insert(meta, Self::box_method_mut(method));
    }

    /// Add a metamethod as a function which accepts generic arguments.
    /// Metamethods in Lua for binary operators can be triggered if either the
    /// left or right argument to the binary operator has a metatable, so the
    /// first argument here is not necessarily a userdata of type T.
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
            Err(
                LuaConversionError::FromLua(
                    "No userdata supplied as first argument to method".to_owned(),
                ).into(),
            )
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
            Err(
                LuaConversionError::FromLua(
                    "No userdata supplied as first argument to method".to_owned(),
                ).into(),
            )
        })

    }
}

/// Trait for custom userdata types.
pub trait LuaUserDataType: 'static + Sized {
    /// Adds custom methods and operators specific to this userdata.
    fn add_methods(_methods: &mut LuaUserDataMethods<Self>) {}
}

/// Handle to an internal instance of custom userdata.  All userdata in this API
/// is based around `RefCell`, to best match the mutable semantics of the Lua
/// language.
#[derive(Clone, Debug)]
pub struct LuaUserData<'lua>(LuaRef<'lua>);

impl<'lua> LuaUserData<'lua> {
    /// Checks whether `T` is the type of this userdata.
    pub fn is<T: LuaUserDataType>(&self) -> bool {
        self.inspect(|_: &RefCell<T>| Ok(())).is_ok()
    }

    /// Borrow this userdata out of the internal RefCell that is held in lua.
    pub fn borrow<T: LuaUserDataType>(&self) -> LuaResult<Ref<T>> {
        self.inspect(|cell| {
            Ok(
                cell.try_borrow().map_err(|_| LuaUserDataError::BorrowError)?,
            )
        })
    }

    /// Borrow mutably this userdata out of the internal RefCell that is held in lua.
    pub fn borrow_mut<T: LuaUserDataType>(&self) -> LuaResult<RefMut<T>> {
        self.inspect(|cell| {
            Ok(cell.try_borrow_mut().map_err(
                |_| LuaUserDataError::BorrowError,
            )?)
        })
    }

    fn inspect<'a, T, R, F>(&'a self, func: F) -> LuaResult<R>
    where
        T: LuaUserDataType,
        F: FnOnce(&'a RefCell<T>) -> LuaResult<R>,
    {
        unsafe {
            let lua = self.0.lua;
            stack_guard(lua.state, 0, move || {
                check_stack(lua.state, 3)?;

                lua.push_ref(lua.state, &self.0);
                let userdata = ffi::lua_touserdata(lua.state, -1);
                assert!(!userdata.is_null());

                if ffi::lua_getmetatable(lua.state, -1) == 0 {
                    return Err(LuaUserDataError::TypeMismatch.into());
                }

                ffi::lua_rawgeti(
                    lua.state,
                    ffi::LUA_REGISTRYINDEX,
                    lua.userdata_metatable::<T>()? as ffi::lua_Integer,
                );
                if ffi::lua_rawequal(lua.state, -1, -2) == 0 {
                    return Err(LuaUserDataError::TypeMismatch.into());
                }

                let res = func(&*(userdata as *const RefCell<T>));

                ffi::lua_pop(lua.state, 3);
                res
            })
        }
    }
}

/// Top level Lua struct which holds the Lua state itself.
pub struct Lua {
    state: *mut ffi::lua_State,
    main_state: *mut ffi::lua_State,
    ephemeral: bool,
}

impl Drop for Lua {
    fn drop(&mut self) {
        unsafe {
            if !self.ephemeral {
                ffi::lua_close(self.state);
            }
        }
    }
}

impl Lua {
    /// Creates a new Lua state.
    ///
    /// Also loads the standard library.
    pub fn new() -> Lua {
        unsafe {
            let state = ffi::luaL_newstate();
            ffi::luaL_openlibs(state);

            stack_guard(state, 0, || {
                ffi::lua_pushlightuserdata(
                    state,
                    &LUA_USERDATA_REGISTRY_KEY as *const u8 as *mut c_void,
                );

                let registered_userdata = ffi::lua_newuserdata(
                    state,
                    mem::size_of::<RefCell<HashMap<TypeId, c_int>>>(),
                ) as *mut RefCell<HashMap<TypeId, c_int>>;
                ptr::write(registered_userdata, RefCell::new(HashMap::new()));

                ffi::lua_newtable(state);

                push_string(state, "__gc");
                ffi::lua_pushcfunction(state, destructor::<RefCell<HashMap<TypeId, c_int>>>);
                ffi::lua_rawset(state, -3);

                ffi::lua_setmetatable(state, -2);

                ffi::lua_rawset(state, ffi::LUA_REGISTRYINDEX);
                Ok(())
            }).unwrap();

            stack_guard(state, 0, || {
                ffi::lua_pushlightuserdata(
                    state,
                    &FUNCTION_METATABLE_REGISTRY_KEY as *const u8 as *mut c_void,
                );

                ffi::lua_newtable(state);

                push_string(state, "__gc");
                ffi::lua_pushcfunction(state, destructor::<LuaCallback>);
                ffi::lua_rawset(state, -3);

                push_string(state, "__metatable");
                ffi::lua_pushboolean(state, 0);
                ffi::lua_rawset(state, -3);

                ffi::lua_rawset(state, ffi::LUA_REGISTRYINDEX);
                Ok(())
            }).unwrap();

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
            }).unwrap();

            Lua {
                state,
                main_state: state,
                ephemeral: false,
            }
        }
    }

    /// Loads a chunk of Lua code and returns it as a function.
    ///
    /// The source can be named by setting the `name` parameter. This is
    /// generally recommended as it results in better error traces.
    ///
    /// Equivalent to Lua's `load` function.
    pub fn load(&self, source: &str, name: Option<&str>) -> LuaResult<LuaFunction> {
        unsafe {
            stack_guard(self.state, 0, || {
                handle_error(
                    self.state,
                    if let Some(name) = name {
                        let name = CString::new(name.to_owned()).map_err(|e| {
                            LuaConversionError::NulError(e)
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
                    },
                )?;

                Ok(LuaFunction(self.pop_ref(self.state)))
            })
        }
    }

    /// Execute a chunk of Lua code.
    ///
    /// This is equivalent to simply loading the source with `load` and then
    /// calling the resulting function with no arguments.
    ///
    /// Returns the values returned by the chunk.
    pub fn exec<'lua, R: FromLuaMulti<'lua>>(
        &'lua self,
        source: &str,
        name: Option<&str>,
    ) -> LuaResult<R> {
        self.load(source, name)?.call(())
    }

    /// Evaluate the given expression or chunk inside this Lua state.
    ///
    /// If `source` is an expression, returns the value it evaluates
    /// to. Otherwise, returns the values returned by the chunk (if any).
    pub fn eval<'lua, R: FromLuaMulti<'lua>>(&'lua self, source: &str) -> LuaResult<R> {
        unsafe {
            stack_guard(self.state, 0, || {
                // First, try interpreting the lua as an expression by adding
                // "return", then as a statement.  This is the same thing the
                // actual lua repl does.
                let return_source = "return ".to_owned() + source;
                let mut res = ffi::luaL_loadbuffer(
                    self.state,
                    return_source.as_ptr() as *const c_char,
                    return_source.len(),
                    ptr::null(),
                );
                if res == ffi::LUA_ERRSYNTAX {
                    ffi::lua_pop(self.state, 1);
                    res = ffi::luaL_loadbuffer(
                        self.state,
                        source.as_ptr() as *const c_char,
                        source.len(),
                        ptr::null(),
                    );
                }

                handle_error(self.state, res)?;
                LuaFunction(self.pop_ref(self.state)).call(())
            })
        }
    }

    /// Pass a `&str` slice to Lua, creating and returning a interned Lua string.
    pub fn create_string(&self, s: &str) -> LuaResult<LuaString> {
        unsafe {
            stack_guard(self.state, 0, || {
                check_stack(self.state, 1)?;
                ffi::lua_pushlstring(self.state, s.as_ptr() as *const c_char, s.len());
                Ok(LuaString(self.pop_ref(self.state)))
            })
        }
    }

    /// Creates and returns a new table.
    pub fn create_table(&self) -> LuaResult<LuaTable> {
        unsafe {
            stack_guard(self.state, 0, || {
                check_stack(self.state, 1)?;
                ffi::lua_newtable(self.state);
                Ok(LuaTable(self.pop_ref(self.state)))
            })
        }
    }

    /// Creates a table and fills it with values from an iterator.
    pub fn create_table_from<'lua, K, V, I>(&'lua self, cont: I) -> LuaResult<LuaTable>
    where
        K: ToLua<'lua>,
        V: ToLua<'lua>,
        I: IntoIterator<Item = (K, V)>,
    {
        unsafe {
            stack_guard(self.state, 0, || {
                check_stack(self.state, 3)?;
                ffi::lua_newtable(self.state);

                for (k, v) in cont {
                    self.push_value(self.state, k.to_lua(self)?);
                    self.push_value(self.state, v.to_lua(self)?);
                    ffi::lua_rawset(self.state, -3);
                }
                Ok(LuaTable(self.pop_ref(self.state)))
            })
        }
    }

    /// Creates a table from an iterator of values, using `1..` as the keys.
    pub fn create_sequence_from<'lua, T, I>(&'lua self, cont: I) -> LuaResult<LuaTable>
    where
        T: ToLua<'lua>,
        I: IntoIterator<Item = T>,
    {
        self.create_table_from(cont.into_iter().enumerate().map(|(k, v)| (k + 1, v)))
    }

    /// Wraps a Rust function or closure, creating a callable Lua function handle to it.
    pub fn create_function<F>(&self, func: F) -> LuaResult<LuaFunction>
    where
        F: 'static + for<'a> FnMut(&'a Lua, LuaMultiValue<'a>) -> LuaResult<LuaMultiValue<'a>>,
    {
        self.create_callback_function(Box::new(func))
    }

    /// Wraps a Lua function into a new thread (or coroutine).
    ///
    /// Equivalent to `coroutine.create`.
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

    /// Create a Lua userdata object from a custom userdata type.
    pub fn create_userdata<T>(&self, data: T) -> LuaResult<LuaUserData>
    where
        T: LuaUserDataType,
    {
        unsafe {
            stack_guard(self.state, 0, move || {
                check_stack(self.state, 2)?;

                let data = RefCell::new(data);
                let data_userdata =
                    ffi::lua_newuserdata(self.state, mem::size_of::<RefCell<T>>()) as
                        *mut RefCell<T>;
                ptr::write(data_userdata, data);

                ffi::lua_rawgeti(
                    self.state,
                    ffi::LUA_REGISTRYINDEX,
                    self.userdata_metatable::<T>()? as ffi::lua_Integer,
                );

                ffi::lua_setmetatable(self.state, -2);

                Ok(LuaUserData(self.pop_ref(self.state)))
            })
        }
    }

    /// Returns a handle to the global environment.
    pub fn globals(&self) -> LuaResult<LuaTable> {
        unsafe {
            check_stack(self.state, 1)?;
            ffi::lua_rawgeti(self.state, ffi::LUA_REGISTRYINDEX, ffi::LUA_RIDX_GLOBALS);
            Ok(LuaTable(self.pop_ref(self.state)))
        }
    }

    /// Coerces a Lua value to a string.
    ///
    /// The value must be a string (in which case this is a no-op) or a number.
    pub fn coerce_string<'lua>(&'lua self, v: LuaValue<'lua>) -> LuaResult<LuaString<'lua>> {
        match v {
            LuaValue::String(s) => Ok(s),
            v => unsafe {
                stack_guard(self.state, 0, || {
                    check_stack(self.state, 1)?;
                    self.push_value(self.state, v);
                    if ffi::lua_tostring(self.state, -1).is_null() {
                        Err(
                            LuaConversionError::FromLua(
                                "cannot convert lua value to string".to_owned(),
                            ).into(),
                        )
                    } else {
                        Ok(LuaString(self.pop_ref(self.state)))
                    }
                })
            },
        }
    }

    /// Coerces a Lua value to an integer.
    ///
    /// The value must be an integer, or a floating point number or a string that can be converted
    /// to an integer. Refer to the Lua manual for details.
    pub fn coerce_integer(&self, v: LuaValue) -> LuaResult<LuaInteger> {
        match v {
            LuaValue::Integer(i) => Ok(i),
            v => unsafe {
                stack_guard(self.state, 0, || {
                    check_stack(self.state, 1)?;
                    self.push_value(self.state, v);
                    let mut isint = 0;
                    let i = ffi::lua_tointegerx(self.state, -1, &mut isint);
                    if isint == 0 {
                        Err(
                            LuaConversionError::FromLua(
                                "cannot convert lua value to integer".to_owned(),
                            ).into(),
                        )
                    } else {
                        ffi::lua_pop(self.state, 1);
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
    pub fn coerce_number(&self, v: LuaValue) -> LuaResult<LuaNumber> {
        match v {
            LuaValue::Number(n) => Ok(n),
            v => unsafe {
                stack_guard(self.state, 0, || {
                    check_stack(self.state, 1)?;
                    self.push_value(self.state, v);
                    let mut isnum = 0;
                    let n = ffi::lua_tonumberx(self.state, -1, &mut isnum);
                    if isnum == 0 {
                        Err(
                            LuaConversionError::FromLua(
                                "cannot convert lua value to number".to_owned(),
                            ).into(),
                        )
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

    /// Packs up a value that implements `ToLuaMulti` into a `LuaMultiValue` instance.
    ///
    /// This can be used to return arbitrary Lua values from a Rust function back to Lua.
    pub fn pack<'lua, T: ToLuaMulti<'lua>>(&'lua self, t: T) -> LuaResult<LuaMultiValue<'lua>> {
        t.to_lua_multi(self)
    }

    /// Unpacks a `LuaMultiValue` instance into a value that implements `FromLuaMulti`.
    ///
    /// This can be used to convert the arguments of a Rust function called by Lua.
    pub fn unpack<'lua, T: FromLuaMulti<'lua>>(
        &'lua self,
        value: LuaMultiValue<'lua>,
    ) -> LuaResult<T> {
        T::from_lua_multi(value, self)
    }

    fn create_callback_function(&self, func: LuaCallback) -> LuaResult<LuaFunction> {
        unsafe extern "C" fn callback_call_impl(state: *mut ffi::lua_State) -> c_int {
            callback_error(state, || {
                let lua = Lua {
                    state: state,
                    main_state: main_state(state),
                    ephemeral: true,
                };

                let func = &mut *(ffi::lua_touserdata(state, ffi::lua_upvalueindex(1)) as
                                      *mut LuaCallback);

                let nargs = ffi::lua_gettop(state);
                let mut args = LuaMultiValue::new();
                for _ in 0..nargs {
                    args.push_front(lua.pop_value(state));
                }

                let results = func(&lua, args)?;
                let nresults = results.len() as c_int;

                for r in results {
                    lua.push_value(state, r);
                }

                Ok(nresults)
            })
        }

        unsafe {
            stack_guard(self.state, 0, move || {
                check_stack(self.state, 2)?;

                let func_userdata =
                    ffi::lua_newuserdata(self.state, mem::size_of::<LuaCallback>()) as
                        *mut LuaCallback;
                ptr::write(func_userdata, func);

                ffi::lua_pushlightuserdata(
                    self.state,
                    &FUNCTION_METATABLE_REGISTRY_KEY as *const u8 as *mut c_void,
                );
                ffi::lua_gettable(self.state, ffi::LUA_REGISTRYINDEX);
                ffi::lua_setmetatable(self.state, -2);

                ffi::lua_pushcclosure(self.state, callback_call_impl, 1);

                Ok(LuaFunction(self.pop_ref(self.state)))
            })
        }
    }

    unsafe fn push_value(&self, state: *mut ffi::lua_State, value: LuaValue) {
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
    }

    unsafe fn pop_value(&self, state: *mut ffi::lua_State) -> LuaValue {
        match ffi::lua_type(state, -1) {
            ffi::LUA_TNIL => {
                ffi::lua_pop(state, 1);
                LuaNil
            }

            ffi::LUA_TBOOLEAN => {
                let b = LuaValue::Boolean(ffi::lua_toboolean(state, -1) != 0);
                ffi::lua_pop(state, 1);
                b
            }

            ffi::LUA_TLIGHTUSERDATA => {
                let ud = LuaValue::LightUserData(LightUserData(ffi::lua_touserdata(state, -1)));
                ffi::lua_pop(state, 1);
                ud
            }

            ffi::LUA_TNUMBER => {
                if ffi::lua_isinteger(state, -1) != 0 {
                    let i = LuaValue::Integer(ffi::lua_tointeger(state, -1));
                    ffi::lua_pop(state, 1);
                    i
                } else {
                    let n = LuaValue::Number(ffi::lua_tonumber(state, -1));
                    ffi::lua_pop(state, 1);
                    n
                }
            }

            ffi::LUA_TSTRING => LuaValue::String(LuaString(self.pop_ref(state))),

            ffi::LUA_TTABLE => LuaValue::Table(LuaTable(self.pop_ref(state))),

            ffi::LUA_TFUNCTION => LuaValue::Function(LuaFunction(self.pop_ref(state))),

            ffi::LUA_TUSERDATA => LuaValue::UserData(LuaUserData(self.pop_ref(state))),

            ffi::LUA_TTHREAD => LuaValue::Thread(LuaThread(self.pop_ref(state))),

            _ => panic!("LUA_TNONE in pop_value"),
        }
    }

    unsafe fn push_ref(&self, state: *mut ffi::lua_State, lref: &LuaRef) {
        assert_eq!(
            lref.lua.main_state,
            self.main_state,
            "Lua instance passed LuaValue created from a different Lua"
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

            ffi::lua_pushlightuserdata(
                self.state,
                &LUA_USERDATA_REGISTRY_KEY as *const u8 as *mut c_void,
            );
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
                            self.push_value(
                                self.state,
                                LuaValue::Function(self.create_callback_function(m)?),
                            );
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
                            self.push_value(
                                self.state,
                                LuaValue::Function(self.create_callback_function(m)?),
                            );
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
                                LuaMetaMethod::IDiv => "__idiv",
                                LuaMetaMethod::BAnd => "__band",
                                LuaMetaMethod::BOr => "__bor",
                                LuaMetaMethod::BXor => "__bxor",
                                LuaMetaMethod::BNot => "__bnot",
                                LuaMetaMethod::Shl => "__shl",
                                LuaMetaMethod::Shr => "__shr",
                                LuaMetaMethod::Concat => "__concat",
                                LuaMetaMethod::Len => "__len",
                                LuaMetaMethod::Eq => "__eq",
                                LuaMetaMethod::Lt => "__lt",
                                LuaMetaMethod::Le => "__le",
                                LuaMetaMethod::Index => "__index",
                                LuaMetaMethod::NewIndex => "__newIndex",
                                LuaMetaMethod::Call => "__call",
                                LuaMetaMethod::ToString => "__tostring",
                            };
                            push_string(self.state, name);
                            self.push_value(
                                self.state,
                                LuaValue::Function(self.create_callback_function(m)?),
                            );
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
