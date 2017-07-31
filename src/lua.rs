use std::{fmt, ptr, slice, str};
use std::ops::{Deref, DerefMut};
use std::iter::FromIterator;
use std::cell::{RefCell, Ref, RefMut};
use std::ffi::CString;
use std::any::TypeId;
use std::marker::PhantomData;
use std::collections::{HashMap, VecDeque};
use std::collections::hash_map::Entry as HashMapEntry;
use std::os::raw::{c_char, c_int, c_void};
use std::string::String as StdString;

use ffi;
use error::*;
use util::*;

/// A dynamically typed Lua value.
#[derive(Debug, Clone)]
pub enum Value<'lua> {
    /// The Lua value `nil`.
    Nil,
    /// The Lua value `true` or `false`.
    Boolean(bool),
    /// A "light userdata" object, equivalent to a raw pointer.
    LightUserData(LightUserData),
    /// An integer number.
    ///
    /// Any Lua number convertible to a `Integer` will be represented as this variant.
    Integer(Integer),
    /// A floating point number.
    Number(Number),
    /// An interned string, managed by Lua.
    ///
    /// Unlike Rust strings, Lua strings may not be valid UTF-8.
    String(String<'lua>),
    /// Reference to a Lua table.
    Table(Table<'lua>),
    /// Reference to a Lua function (or closure).
    Function(Function<'lua>),
    /// Reference to a Lua thread (or coroutine).
    Thread(Thread<'lua>),
    /// Reference to a userdata object that holds a custom type which implements
    /// `UserData`.  Special builtin userdata types will be represented as
    /// other `Value` variants.
    UserData(AnyUserData<'lua>),
    /// `Error` is a special builtin userdata type.  When received from Lua
    /// it is implicitly cloned.
    Error(Error),
}
pub use self::Value::Nil;

/// Trait for types convertible to `Value`.
pub trait ToLua<'a> {
    /// Performs the conversion.
    fn to_lua(self, lua: &'a Lua) -> Result<Value<'a>>;
}

/// Trait for types convertible from `Value`.
pub trait FromLua<'a>: Sized {
    /// Performs the conversion.
    fn from_lua(lua_value: Value<'a>, lua: &'a Lua) -> Result<Self>;
}

/// Multiple Lua values used for both argument passing and also for multiple return values.
#[derive(Debug, Clone)]
pub struct MultiValue<'lua>(VecDeque<Value<'lua>>);

impl<'lua> MultiValue<'lua> {
    pub fn new() -> MultiValue<'lua> {
        MultiValue(VecDeque::new())
    }
}

impl<'lua> FromIterator<Value<'lua>> for MultiValue<'lua> {
    fn from_iter<I: IntoIterator<Item = Value<'lua>>>(iter: I) -> Self {
        MultiValue(VecDeque::from_iter(iter))
    }
}

impl<'lua> IntoIterator for MultiValue<'lua> {
    type Item = Value<'lua>;
    type IntoIter = <VecDeque<Value<'lua>> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'lua> Deref for MultiValue<'lua> {
    type Target = VecDeque<Value<'lua>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'lua> DerefMut for MultiValue<'lua> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Trait for types convertible to any number of Lua values.
///
/// This is a generalization of `ToLua`, allowing any number of resulting Lua values instead of just
/// one. Any type that implements `ToLua` will automatically implement this trait.
pub trait ToLuaMulti<'a> {
    /// Performs the conversion.
    fn to_lua_multi(self, lua: &'a Lua) -> Result<MultiValue<'a>>;
}

/// Trait for types that can be created from an arbitrary number of Lua values.
///
/// This is a generalization of `FromLua`, allowing an arbitrary number of Lua values to participate
/// in the conversion. Any type that implements `FromLua` will automatically implement this trait.
pub trait FromLuaMulti<'a>: Sized {
    /// Performs the conversion.
    ///
    /// In case `values` contains more values than needed to perform the conversion, the excess
    /// values should be ignored. This reflects the semantics of Lua when calling a function or
    /// assigning values. Similarly, if not enough values are given, conversions should assume that
    /// any missing values are nil.
    fn from_lua_multi(values: MultiValue<'a>, lua: &'a Lua) -> Result<Self>;
}

type Callback<'lua> = Box<FnMut(&'lua Lua, MultiValue<'lua>) -> Result<MultiValue<'lua>> + 'lua>;

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
pub type Integer = ffi::lua_Integer;
/// Type of Lua floating point numbers.
pub type Number = ffi::lua_Number;

/// A "light" userdata value. Equivalent to an unmanaged raw pointer.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct LightUserData(pub *mut c_void);

/// Handle to an internal Lua string.
///
/// Unlike Rust strings, Lua strings may not be valid UTF-8.
#[derive(Clone, Debug)]
pub struct String<'lua>(LuaRef<'lua>);

impl<'lua> String<'lua> {
    /// Get a `&str` slice if the Lua string is valid UTF-8.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate rlua;
    /// # use rlua::{Lua, String, Result};
    /// # fn try_main() -> Result<()> {
    /// let lua = Lua::new();
    /// let globals = lua.globals();
    ///
    /// let version: String = globals.get("_VERSION")?;
    /// assert!(version.to_str().unwrap().contains("Lua"));
    ///
    /// let non_utf8: String = lua.eval(r#"  "test\xff"  "#, None)?;
    /// assert!(non_utf8.to_str().is_err());
    /// # Ok(())
    /// # }
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    pub fn to_str(&self) -> Result<&str> {
        str::from_utf8(self.as_bytes()).map_err(|e| Error::FromLuaConversionError(e.to_string()))
    }

    /// Get the bytes that make up this string.
    ///
    /// The returned slice will not contain the terminating null byte, but will contain any null
    /// bytes embedded into the Lua string.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate rlua;
    /// # use rlua::{Lua, String};
    /// # fn main() {
    /// let lua = Lua::new();
    ///
    /// let non_utf8: String = lua.eval(r#"  "test\xff"  "#, None).unwrap();
    /// assert!(non_utf8.to_str().is_err());    // oh no :(
    /// assert_eq!(non_utf8.as_bytes(), &b"test\xff"[..]);
    /// # }
    /// ```
    pub fn as_bytes(&self) -> &[u8] {
        let lua = self.0.lua;
        unsafe {
            stack_guard(lua.state, 0, || {
                check_stack(lua.state, 1);
                lua.push_ref(lua.state, &self.0);
                assert_eq!(ffi::lua_type(lua.state, -1), ffi::LUA_TSTRING);

                let mut size = 0;
                let data = ffi::lua_tolstring(lua.state, -1, &mut size);

                ffi::lua_pop(lua.state, 1);
                slice::from_raw_parts(data as *const u8, size)
            })
        }
    }
}

/// Handle to an internal Lua table.
#[derive(Clone, Debug)]
pub struct Table<'lua>(LuaRef<'lua>);

impl<'lua> Table<'lua> {
    /// Sets a key-value pair in the table.
    ///
    /// If the value is `nil`, this will effectively remove the pair.
    ///
    /// This might invoke the `__newindex` metamethod. Use the [`raw_set`] method if that is not
    /// desired.
    ///
    /// # Examples
    ///
    /// Export a value as a global to make it usable from Lua:
    ///
    /// ```
    /// # extern crate rlua;
    /// # use rlua::{Lua, Result};
    /// # fn try_main() -> Result<()> {
    /// let lua = Lua::new();
    /// let globals = lua.globals();
    ///
    /// globals.set("assertions", cfg!(debug_assertions))?;
    ///
    /// lua.exec::<()>(r#"
    ///     if assertions == true then
    ///         -- ...
    ///     elseif assertions == false then
    ///         -- ...
    ///     else
    ///         error("assertions neither on nor off?")
    ///     end
    /// "#, None)?;
    /// # Ok(())
    /// # }
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    ///
    /// [`raw_set`]: #method.raw_set
    pub fn set<K: ToLua<'lua>, V: ToLua<'lua>>(&self, key: K, value: V) -> Result<()> {
        let lua = self.0.lua;
        unsafe {
            stack_err_guard(lua.state, 0, || {
                check_stack(lua.state, 7);
                lua.push_ref(lua.state, &self.0);
                lua.push_value(lua.state, key.to_lua(lua)?);
                lua.push_value(lua.state, value.to_lua(lua)?);
                psettable(lua.state, -3)?;
                ffi::lua_pop(lua.state, 1);
                Ok(())
            })
        }
    }

    /// Gets the value associated to `key` from the table.
    ///
    /// If no value is associated to `key`, returns the `nil` value.
    ///
    /// This might invoke the `__index` metamethod. Use the [`raw_get`] method if that is not
    /// desired.
    ///
    /// # Examples
    ///
    /// Query the version of the Lua interpreter:
    ///
    /// ```
    /// # extern crate rlua;
    /// # use rlua::{Lua, Result};
    /// # fn try_main() -> Result<()> {
    /// let lua = Lua::new();
    /// let globals = lua.globals();
    ///
    /// let version: String = globals.get("_VERSION")?;
    /// println!("Lua version: {}", version);
    /// # Ok(())
    /// # }
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    ///
    /// [`raw_get`]: #method.raw_get
    pub fn get<K: ToLua<'lua>, V: FromLua<'lua>>(&self, key: K) -> Result<V> {
        let lua = self.0.lua;
        unsafe {
            stack_err_guard(lua.state, 0, || {
                check_stack(lua.state, 5);
                lua.push_ref(lua.state, &self.0);
                lua.push_value(lua.state, key.to_lua(lua)?);
                pgettable(lua.state, -2)?;
                let res = lua.pop_value(lua.state);
                ffi::lua_pop(lua.state, 1);
                V::from_lua(res, lua)
            })
        }
    }

    /// Checks whether the table contains a non-nil value for `key`.
    pub fn contains_key<K: ToLua<'lua>>(&self, key: K) -> Result<bool> {
        let lua = self.0.lua;
        unsafe {
            stack_err_guard(lua.state, 0, || {
                check_stack(lua.state, 5);
                lua.push_ref(lua.state, &self.0);
                lua.push_value(lua.state, key.to_lua(lua)?);
                pgettable(lua.state, -2)?;
                let has = ffi::lua_isnil(lua.state, -1) == 0;
                ffi::lua_pop(lua.state, 2);
                Ok(has)
            })
        }
    }

    /// Sets a key-value pair without invoking metamethods.
    pub fn raw_set<K: ToLua<'lua>, V: ToLua<'lua>>(&self, key: K, value: V) -> Result<()> {
        let lua = self.0.lua;
        unsafe {
            stack_err_guard(lua.state, 0, || {
                check_stack(lua.state, 3);
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
    pub fn raw_get<K: ToLua<'lua>, V: FromLua<'lua>>(&self, key: K) -> Result<V> {
        let lua = self.0.lua;
        unsafe {
            stack_err_guard(lua.state, 0, || {
                check_stack(lua.state, 2);
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
    /// This might invoke the `__len` metamethod. Use the [`raw_len`] method if that is not desired.
    ///
    /// [`raw_len`]: #method.raw_len
    pub fn len(&self) -> Result<Integer> {
        let lua = self.0.lua;
        unsafe {
            stack_err_guard(lua.state, 0, || {
                check_stack(lua.state, 3);
                lua.push_ref(lua.state, &self.0);
                let len = plen(lua.state, -1)?;
                ffi::lua_pop(lua.state, 1);
                Ok(len)
            })
        }
    }

    /// Returns the result of the Lua `#` operator, without invoking the `__len` metamethod.
    pub fn raw_len(&self) -> Integer {
        let lua = self.0.lua;
        unsafe {
            stack_guard(lua.state, 0, || {
                check_stack(lua.state, 1);
                lua.push_ref(lua.state, &self.0);
                let len = ffi::lua_rawlen(lua.state, -1);
                ffi::lua_pop(lua.state, 1);
                len as Integer
            })
        }
    }

    /// Consume this table and return an iterator over the pairs of the table.
    ///
    /// This works like the Lua `pairs` function, but does not invoke the `__pairs` metamethod.
    ///
    /// The pairs are wrapped in a [`Result`], since they are lazily converted to `K` and `V` types.
    ///
    /// # Note
    ///
    /// While this method consumes the `Table` object, it can not prevent code from mutating the
    /// table while the iteration is in progress. Refer to the [Lua manual] for information about
    /// the consequences of such mutation.
    ///
    /// # Examples
    ///
    /// Iterate over all globals:
    ///
    /// ```
    /// # extern crate rlua;
    /// # use rlua::{Lua, Result, Value};
    /// # fn try_main() -> Result<()> {
    /// let lua = Lua::new();
    /// let globals = lua.globals();
    ///
    /// for pair in globals.pairs::<Value, Value>() {
    ///     let (key, value) = pair?;
    /// #   let _ = (key, value);   // used
    ///     // ...
    /// }
    /// # Ok(())
    /// # }
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    ///
    /// [`Result`]: type.Result.html
    /// [Lua manual]: http://www.lua.org/manual/5.3/manual.html#pdf-next
    pub fn pairs<K: FromLua<'lua>, V: FromLua<'lua>>(self) -> TablePairs<'lua, K, V> {
        let next_key = Some(LuaRef {
            lua: self.0.lua,
            registry_id: ffi::LUA_REFNIL,
        });

        TablePairs {
            table: self.0,
            next_key,
            _phantom: PhantomData,
        }
    }

    /// Consume this table and return an iterator over all values in the sequence part of the table.
    ///
    /// The iterator will yield all values `t[1]`, `t[2]`, and so on, until a `nil` value is
    /// encountered. This mirrors the behaviour of Lua's `ipairs` function and will invoke the
    /// `__index` metamethod according to the usual rules. However, the deprecated `__ipairs`
    /// metatable will not be called.
    ///
    /// Just like [`pairs`], the values are wrapped in a [`Result`].
    ///
    /// # Note
    ///
    /// While this method consumes the `Table` object, it can not prevent code from mutating the
    /// table while the iteration is in progress. Refer to the [Lua manual] for information about
    /// the consequences of such mutation.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate rlua;
    /// # use rlua::{Lua, Result, Table};
    /// # fn try_main() -> Result<()> {
    /// let lua = Lua::new();
    /// let my_table: Table = lua.eval("{ [1] = 4, [2] = 5, [4] = 7, key = 2 }", None)?;
    ///
    /// let expected = [4, 5];
    /// for (&expected, got) in expected.iter().zip(my_table.sequence_values::<u32>()) {
    ///     assert_eq!(expected, got?);
    /// }
    /// # Ok(())
    /// # }
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    ///
    /// [`pairs`]: #method.pairs
    /// [`Result`]: type.Result.html
    /// [Lua manual]: http://www.lua.org/manual/5.3/manual.html#pdf-next
    pub fn sequence_values<V: FromLua<'lua>>(self) -> TableSequence<'lua, V> {
        TableSequence {
            table: self.0,
            index: Some(1),
            _phantom: PhantomData,
        }
    }
}

/// An iterator over the pairs of a Lua table.
///
/// This struct is created by the [`Table::pairs`] method.
///
/// [`Table::pairs`]: struct.Table.html#method.pairs
pub struct TablePairs<'lua, K, V> {
    table: LuaRef<'lua>,
    next_key: Option<LuaRef<'lua>>,
    _phantom: PhantomData<(K, V)>,
}

impl<'lua, K, V> Iterator for TablePairs<'lua, K, V>
where
    K: FromLua<'lua>,
    V: FromLua<'lua>,
{
    type Item = Result<(K, V)>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(next_key) = self.next_key.take() {
            let lua = self.table.lua;

            unsafe {
                stack_guard(lua.state, 0, || {
                    check_stack(lua.state, 6);

                    lua.push_ref(lua.state, &self.table);
                    lua.push_ref(lua.state, &next_key);

                    match pnext(lua.state, -2) {
                        Ok(0) => {
                            ffi::lua_pop(lua.state, 1);
                            None
                        }
                        Ok(_) => {
                            ffi::lua_pushvalue(lua.state, -2);
                            let key = lua.pop_value(lua.state);
                            let value = lua.pop_value(lua.state);
                            self.next_key = Some(lua.pop_ref(lua.state));
                            ffi::lua_pop(lua.state, 1);

                            Some((|| {
                                 let key = K::from_lua(key, lua)?;
                                 let value = V::from_lua(value, lua)?;
                                 Ok((key, value))
                             })())
                        }
                        Err(e) => Some(Err(e)),
                    }
                })
            }
        } else {
            None
        }
    }
}

/// An iterator over the sequence part of a Lua table.
///
/// This struct is created by the [`Table::sequence_values`] method.
///
/// [`Table::sequence_values`]: struct.Table.html#method.sequence_values
pub struct TableSequence<'lua, V> {
    table: LuaRef<'lua>,
    index: Option<Integer>,
    _phantom: PhantomData<V>,
}

impl<'lua, V> Iterator for TableSequence<'lua, V>
where
    V: FromLua<'lua>,
{
    type Item = Result<V>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(index) = self.index.take() {
            let lua = self.table.lua;

            unsafe {
                stack_guard(lua.state, 0, || {
                    check_stack(lua.state, 4);

                    lua.push_ref(lua.state, &self.table);
                    match pgeti(lua.state, -1, index) {
                        Ok(ffi::LUA_TNIL) => {
                            ffi::lua_pop(lua.state, 2);
                            None
                        }
                        Ok(_) => {
                            let value = lua.pop_value(lua.state);
                            ffi::lua_pop(lua.state, 1);
                            self.index = Some(index + 1);
                            Some(V::from_lua(value, lua))
                        }
                        Err(err) => Some(Err(err)),
                    }
                })
            }
        } else {
            None
        }
    }
}

/// Handle to an internal Lua function.
#[derive(Clone, Debug)]
pub struct Function<'lua>(LuaRef<'lua>);

impl<'lua> Function<'lua> {
    /// Calls the function, passing `args` as function arguments.
    ///
    /// The function's return values are converted to the generic type `R`.
    ///
    /// # Examples
    ///
    /// Call Lua's built-in `tostring` function:
    ///
    /// ```
    /// # extern crate rlua;
    /// # use rlua::{Lua, Function, Result};
    /// # fn try_main() -> Result<()> {
    /// let lua = Lua::new();
    /// let globals = lua.globals();
    ///
    /// let tostring: Function = globals.get("tostring")?;
    ///
    /// assert_eq!(tostring.call::<_, String>(123)?, "123");
    ///
    /// # Ok(())
    /// # }
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    ///
    /// Call a function with multiple arguments:
    ///
    /// ```
    /// # extern crate rlua;
    /// # use rlua::{Lua, Function, Result};
    /// # fn try_main() -> Result<()> {
    /// let lua = Lua::new();
    ///
    /// let sum: Function = lua.eval(r#"
    ///     function(a, b)
    ///         return a + b
    ///     end
    /// "#, None)?;
    ///
    /// assert_eq!(sum.call::<_, u32>((3, 4))?, 3 + 4);
    ///
    /// # Ok(())
    /// # }
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    pub fn call<A: ToLuaMulti<'lua>, R: FromLuaMulti<'lua>>(&self, args: A) -> Result<R> {
        let lua = self.0.lua;
        unsafe {
            stack_err_guard(lua.state, 0, || {
                let args = args.to_lua_multi(lua)?;
                let nargs = args.len() as c_int;
                check_stack(lua.state, nargs + 3);

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
                let mut results = MultiValue::new();
                for _ in 0..nresults {
                    results.push_front(lua.pop_value(lua.state));
                }
                R::from_lua_multi(results, lua)
            })
        }
    }

    /// Returns a function that, when called, calls `self`, passing `args` as the first set of
    /// arguments.
    ///
    /// If any arguments are passed to the returned function, they will be passed after `args`.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate rlua;
    /// # use rlua::{Lua, Function, Result};
    /// # fn try_main() -> Result<()> {
    /// let lua = Lua::new();
    ///
    /// let sum: Function = lua.eval(r#"
    ///     function(a, b)
    ///         return a + b
    ///     end
    /// "#, None)?;
    ///
    /// let bound_a = sum.bind(1)?;
    /// assert_eq!(bound_a.call::<_, u32>(2)?, 1 + 2);
    ///
    /// let bound_a_and_b = sum.bind(13)?.bind(57)?;
    /// assert_eq!(bound_a_and_b.call::<_, u32>(())?, 13 + 57);
    ///
    /// # Ok(())
    /// # }
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    pub fn bind<A: ToLuaMulti<'lua>>(&self, args: A) -> Result<Function<'lua>> {
        unsafe extern "C" fn bind_call_impl(state: *mut ffi::lua_State) -> c_int {
            let nargs = ffi::lua_gettop(state);

            let nbinds = ffi::lua_tointeger(state, ffi::lua_upvalueindex(2)) as c_int;
            check_stack(state, nbinds + 1);

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
            stack_err_guard(lua.state, 0, || {
                let args = args.to_lua_multi(lua)?;
                let nargs = args.len() as c_int;

                check_stack(lua.state, nargs + 2);
                lua.push_ref(lua.state, &self.0);
                ffi::lua_pushinteger(lua.state, nargs as ffi::lua_Integer);
                for arg in args {
                    lua.push_value(lua.state, arg);
                }

                ffi::lua_pushcclosure(lua.state, bind_call_impl, nargs + 2);

                Ok(Function(lua.pop_ref(lua.state)))
            })
        }
    }
}

/// Status of a Lua thread (or coroutine).
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ThreadStatus {
    /// The thread has finished executing.
    Dead,
    /// The thread was just created, is currently running or is suspended because it has called
    /// `coroutine.yield`.
    ///
    /// If a thread is in this state, it can be resumed by calling [`Thread::resume`].
    ///
    /// [`Thread::resume`]: struct.Thread.html#method.resume
    Active,
    /// The thread has raised a Lua error during execution.
    Error,
}

/// Handle to an internal Lua thread (or coroutine).
#[derive(Clone, Debug)]
pub struct Thread<'lua>(LuaRef<'lua>);

impl<'lua> Thread<'lua> {
    /// Resumes execution of this thread.
    ///
    /// Equivalent to `coroutine.resume`.
    ///
    /// Passes `args` as arguments to the thread. If the coroutine has called `coroutine.yield`, it
    /// will return these arguments. Otherwise, the coroutine wasn't yet started, so the arguments
    /// are passed to its main function.
    ///
    /// If the thread is no longer in `Active` state (meaning it has finished execution or
    /// encountered an error), this will return `Err(CoroutineInactive)`, otherwise will return `Ok`
    /// as follows:
    ///
    /// If the thread calls `coroutine.yield`, returns the values passed to `yield`. If the thread
    /// `return`s values from its main function, returns those.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate rlua;
    /// # use rlua::{Lua, Thread, Error, Result};
    /// # fn try_main() -> Result<()> {
    /// let lua = Lua::new();
    /// let thread: Thread = lua.eval(r#"
    ///     coroutine.create(function(arg)
    ///         assert(arg == 42)
    ///         local yieldarg = coroutine.yield(123)
    ///         assert(yieldarg == 43)
    ///         return 987
    ///     end)
    /// "#, None).unwrap();
    ///
    /// assert_eq!(thread.resume::<_, u32>(42).unwrap(), 123);
    /// assert_eq!(thread.resume::<_, u32>(43).unwrap(), 987);
    ///
    /// // The coroutine has now returned, so `resume` will fail
    /// match thread.resume::<_, u32>(()) {
    ///     Err(Error::CoroutineInactive) => {},
    ///     unexpected => panic!("unexpected result {:?}", unexpected),
    /// }
    /// # Ok(())
    /// # }
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    pub fn resume<A, R>(&self, args: A) -> Result<R>
    where
        A: ToLuaMulti<'lua>,
        R: FromLuaMulti<'lua>,
    {
        let lua = self.0.lua;
        unsafe {
            stack_err_guard(lua.state, 0, || {
                check_stack(lua.state, 1);

                lua.push_ref(lua.state, &self.0);
                let thread_state = ffi::lua_tothread(lua.state, -1);

                let status = ffi::lua_status(thread_state);
                if status != ffi::LUA_YIELD && ffi::lua_gettop(thread_state) == 0 {
                    return Err(Error::CoroutineInactive);
                }

                ffi::lua_pop(lua.state, 1);

                let args = args.to_lua_multi(lua)?;
                let nargs = args.len() as c_int;
                check_stack(thread_state, nargs);

                for arg in args {
                    lua.push_value(thread_state, arg);
                }

                handle_error(
                    thread_state,
                    resume_with_traceback(thread_state, lua.state, nargs),
                )?;

                let nresults = ffi::lua_gettop(thread_state);
                let mut results = MultiValue::new();
                for _ in 0..nresults {
                    results.push_front(lua.pop_value(thread_state));
                }
                R::from_lua_multi(results, lua)
            })
        }
    }

    /// Gets the status of the thread.
    pub fn status(&self) -> ThreadStatus {
        let lua = self.0.lua;
        unsafe {
            stack_guard(lua.state, 0, || {
                check_stack(lua.state, 1);

                lua.push_ref(lua.state, &self.0);
                let thread_state = ffi::lua_tothread(lua.state, -1);
                ffi::lua_pop(lua.state, 1);

                let status = ffi::lua_status(thread_state);
                if status != ffi::LUA_OK && status != ffi::LUA_YIELD {
                    ThreadStatus::Error
                } else if status == ffi::LUA_YIELD || ffi::lua_gettop(thread_state) > 0 {
                    ThreadStatus::Active
                } else {
                    ThreadStatus::Dead
                }
            })
        }
    }
}

/// Kinds of metamethods that can be overridden.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum MetaMethod {
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

/// Method registry for [`UserData`] implementors.
///
/// [`UserData`]: trait.UserData.html
pub struct UserDataMethods<'lua, T> {
    methods: HashMap<StdString, Callback<'lua>>,
    meta_methods: HashMap<MetaMethod, Callback<'lua>>,
    _type: PhantomData<T>,
}

impl<'lua, T: UserData> UserDataMethods<'lua, T> {
    /// Add a method which accepts a `&T` as the first parameter.
    ///
    /// Regular methods are implemented by overriding the `__index` metamethod and returning the
    /// accessed method. This allows them to be used with the expected `userdata:method()` syntax.
    ///
    /// If `add_meta_method` is used to override the `__index` metamethod, this approach will fall
    /// back to the user-provided metamethod if no regular method was found.
    pub fn add_method<A, R, M>(&mut self, name: &str, method: M)
    where
        A: 'lua + FromLuaMulti<'lua>,
        R: 'lua + ToLuaMulti<'lua>,
        M: 'lua + for<'a> FnMut(&'lua Lua, &'a T, A) -> Result<R>,
    {
        self.methods.insert(
            name.to_owned(),
            Self::box_method(method),
        );
    }

    /// Add a regular method which accepts a `&mut T` as the first parameter.
    ///
    /// Refer to [`add_method`] for more information about the implementation.
    ///
    /// [`add_method`]: #method.add_method
    pub fn add_method_mut<A, R, M>(&mut self, name: &str, method: M)
    where
        A: 'lua + FromLuaMulti<'lua>,
        R: 'lua + ToLuaMulti<'lua>,
        M: 'lua + for<'a> FnMut(&'lua Lua, &'a mut T, A) -> Result<R>,
    {
        self.methods.insert(
            name.to_owned(),
            Self::box_method_mut(method),
        );
    }

    /// Add a regular method as a function which accepts generic arguments, the first argument will
    /// always be a `UserData` of type T.
    ///
    /// Prefer to use [`add_method`] or [`add_method_mut`] as they are easier to use.
    ///
    /// [`add_method`]: #method.add_method
    /// [`add_method_mut`]: #method.add_method_mut
    pub fn add_function<A, R, F>(&mut self, name: &str, function: F)
    where
        A: 'lua + FromLuaMulti<'lua>,
        R: 'lua + ToLuaMulti<'lua>,
        F: 'lua + FnMut(&'lua Lua, A) -> Result<R>,
    {
        self.methods.insert(
            name.to_owned(),
            Self::box_function(function),
        );
    }

    /// Add a metamethod which accepts a `&T` as the first parameter.
    ///
    /// # Note
    ///
    /// This can cause an error with certain binary metamethods that can trigger if only the right
    /// side has a metatable. To prevent this, use [`add_meta_function`].
    ///
    /// [`add_meta_function`]: #method.add_meta_function
    pub fn add_meta_method<A, R, M>(&mut self, meta: MetaMethod, method: M)
    where
        A: 'lua + FromLuaMulti<'lua>,
        R: 'lua + ToLuaMulti<'lua>,
        M: 'lua + for<'a> FnMut(&'lua Lua, &'a T, A) -> Result<R>,
    {
        self.meta_methods.insert(meta, Self::box_method(method));
    }

    /// Add a metamethod as a function which accepts a `&mut T` as the first parameter.
    ///
    /// # Note
    ///
    /// This can cause an error with certain binary metamethods that can trigger if only the right
    /// side has a metatable. To prevent this, use [`add_meta_function`].
    ///
    /// [`add_meta_function`]: #method.add_meta_function
    pub fn add_meta_method_mut<A, R, M>(&mut self, meta: MetaMethod, method: M)
    where
        A: 'lua + FromLuaMulti<'lua>,
        R: 'lua + ToLuaMulti<'lua>,
        M: 'lua + for<'a> FnMut(&'lua Lua, &'a mut T, A) -> Result<R>,
    {
        self.meta_methods.insert(meta, Self::box_method_mut(method));
    }

    /// Add a metamethod which accepts generic arguments.
    ///
    /// Metamethods for binary operators can be triggered if either the left or right argument to
    /// the binary operator has a metatable, so the first argument here is not necessarily a
    /// userdata of type `T`.
    pub fn add_meta_function<A, R, F>(&mut self, meta: MetaMethod, function: F)
    where
        A: 'lua + FromLuaMulti<'lua>,
        R: 'lua + ToLuaMulti<'lua>,
        F: 'lua + FnMut(&'lua Lua, A) -> Result<R>,
    {
        self.meta_methods.insert(meta, Self::box_function(function));
    }

    fn box_function<A, R, F>(mut function: F) -> Callback<'lua>
    where
        A: 'lua + FromLuaMulti<'lua>,
        R: 'lua + ToLuaMulti<'lua>,
        F: 'lua + for<'a> FnMut(&'lua Lua, A) -> Result<R>,
    {
        Box::new(move |lua, args| {
            function(lua, A::from_lua_multi(args, lua)?)?.to_lua_multi(
                lua,
            )
        })
    }

    fn box_method<A, R, M>(mut method: M) -> Callback<'lua>
    where
        A: 'lua + FromLuaMulti<'lua>,
        R: 'lua + ToLuaMulti<'lua>,
        M: 'lua + for<'a> FnMut(&'lua Lua, &'a T, A) -> Result<R>,
    {
        Box::new(move |lua, mut args| if let Some(front) = args.pop_front() {
            let userdata = AnyUserData::from_lua(front, lua)?;
            let userdata = userdata.borrow::<T>()?;
            method(lua, &userdata, A::from_lua_multi(args, lua)?)?
                .to_lua_multi(lua)
        } else {
            Err(Error::FromLuaConversionError(
                "No userdata supplied as first argument to method"
                    .to_owned(),
            ))
        })
    }

    fn box_method_mut<A, R, M>(mut method: M) -> Callback<'lua>
    where
        A: 'lua + FromLuaMulti<'lua>,
        R: 'lua + ToLuaMulti<'lua>,
        M: 'lua + for<'a> FnMut(&'lua Lua, &'a mut T, A) -> Result<R>,
    {
        Box::new(move |lua, mut args| if let Some(front) = args.pop_front() {
            let userdata = AnyUserData::from_lua(front, lua)?;
            let mut userdata = userdata.borrow_mut::<T>()?;
            method(lua, &mut userdata, A::from_lua_multi(args, lua)?)?
                .to_lua_multi(lua)
        } else {
            Err(
                Error::FromLuaConversionError(
                    "No userdata supplied as first argument to method".to_owned(),
                ).into(),
            )
        })
    }
}

/// Trait for custom userdata types.
///
/// By implementing this trait, a struct becomes eligible for use inside Lua code. Implementations
/// of `ToLua` and `FromLua` are automatically provided.
///
/// # Examples
///
/// ```
/// # extern crate rlua;
/// # use rlua::{Lua, UserData, Result};
/// # fn try_main() -> Result<()> {
/// struct MyUserData(i32);
///
/// impl UserData for MyUserData {}
///
/// let lua = Lua::new();
///
/// // `MyUserData` now implements `ToLua`:
/// lua.globals().set("myobject", MyUserData(123))?;
///
/// lua.exec::<()>("assert(type(myobject) == 'userdata')", None)?;
/// # Ok(())
/// # }
/// # fn main() {
/// #     try_main().unwrap();
/// # }
/// ```
///
/// Custom methods and operators can be provided by implementing `add_methods` (refer to
/// [`UserDataMethods`] for more information):
///
/// ```
/// # extern crate rlua;
/// # use rlua::{Lua, MetaMethod, UserData, UserDataMethods, Result};
/// # fn try_main() -> Result<()> {
/// struct MyUserData(i32);
///
/// impl UserData for MyUserData {
///     fn add_methods(methods: &mut UserDataMethods<Self>) {
///         methods.add_method("get", |_, this, _: ()| {
///             Ok(this.0)
///         });
///
///         methods.add_method_mut("add", |_, this, value: i32| {
///             this.0 += value;
///             Ok(())
///         });
///
///         methods.add_meta_method(MetaMethod::Add, |_, this, value: i32| {
///             Ok(this.0 + value)
///         });
///     }
/// }
///
/// let lua = Lua::new();
///
/// lua.globals().set("myobject", MyUserData(123))?;
///
/// lua.exec::<()>(r#"
///     assert(myobject:get() == 123)
///     myobject:add(7)
///     assert(myobject:get() == 130)
///     assert(myobject + 10 == 140)
/// "#, None)?;
/// # Ok(())
/// # }
/// # fn main() {
/// #     try_main().unwrap();
/// # }
/// ```
///
/// [`UserDataMethods`]: struct.UserDataMethods.html
pub trait UserData: 'static + Sized {
    /// Adds custom methods and operators specific to this userdata.
    fn add_methods(_methods: &mut UserDataMethods<Self>) {}
}

/// Handle to an internal Lua userdata for any type that implements [`UserData`].
///
/// Similar to `std::any::Any`, this provides an interface for dynamic type checking via the [`is`]
/// and [`borrow`] methods.
///
/// Internally, instances are stored in a `RefCell`, to best match the mutable semantics of the Lua
/// language.
///
/// # Note
///
/// This API should only be used when necessary. Implementing [`UserData`] already allows defining
/// methods which check the type and acquire a borrow behind the scenes.
///
/// [`UserData`]: trait.UserData.html
/// [`is`]: #method.is
/// [`borrow`]: #method.borrow
#[derive(Clone, Debug)]
pub struct AnyUserData<'lua>(LuaRef<'lua>);

impl<'lua> AnyUserData<'lua> {
    /// Checks whether the type of this userdata is `T`.
    pub fn is<T: UserData>(&self) -> bool {
        self.inspect(|_: &RefCell<T>| ()).is_some()
    }

    /// Borrow this userdata immutably if it is of type `T`.
    ///
    /// # Errors
    ///
    /// Returns a `UserDataBorrowError` if the userdata is already mutably borrowed. Returns a
    /// `UserDataTypeMismatch` if the userdata is not of type `T`.
    pub fn borrow<T: UserData>(&self) -> Result<Ref<T>> {
        self.inspect(|cell| {
            Ok(cell.try_borrow().map_err(|_| Error::UserDataBorrowError)?)
        }).ok_or(Error::UserDataTypeMismatch)?
    }

    /// Borrow this userdata mutably if it is of type `T`.
    ///
    /// # Errors
    ///
    /// Returns a `UserDataBorrowMutError` if the userdata is already borrowed. Returns a
    /// `UserDataTypeMismatch` if the userdata is not of type `T`.
    pub fn borrow_mut<T: UserData>(&self) -> Result<RefMut<T>> {
        self.inspect(|cell| {
            Ok(cell.try_borrow_mut().map_err(
                |_| Error::UserDataBorrowMutError,
            )?)
        }).ok_or(Error::UserDataTypeMismatch)?
    }

    fn inspect<'a, T, R, F>(&'a self, func: F) -> Option<R>
    where
        T: UserData,
        F: FnOnce(&'a RefCell<T>) -> R,
    {
        unsafe {
            let lua = self.0.lua;
            stack_guard(lua.state, 0, move || {
                check_stack(lua.state, 3);

                lua.push_ref(lua.state, &self.0);

                lua_assert!(
                    lua.state,
                    ffi::lua_getmetatable(lua.state, -1) != 0,
                    "AnyUserData missing metatable"
                );

                ffi::lua_rawgeti(
                    lua.state,
                    ffi::LUA_REGISTRYINDEX,
                    lua.userdata_metatable::<T>() as ffi::lua_Integer,
                );

                if ffi::lua_rawequal(lua.state, -1, -2) == 0 {
                    ffi::lua_pop(lua.state, 3);
                    None
                } else {
                    let res = func(&*get_userdata::<RefCell<T>>(lua.state, -3));
                    ffi::lua_pop(lua.state, 3);
                    Some(res)
                }
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

            stack_guard(state, 0, || {
                ffi::luaL_openlibs(state);

                // Create the userdata registry table

                ffi::lua_pushlightuserdata(
                    state,
                    &LUA_USERDATA_REGISTRY_KEY as *const u8 as *mut c_void,
                );

                push_userdata::<RefCell<HashMap<TypeId, c_int>>>(
                    state,
                    RefCell::new(HashMap::new()),
                );

                ffi::lua_newtable(state);

                push_string(state, "__gc");
                ffi::lua_pushcfunction(
                    state,
                    userdata_destructor::<RefCell<HashMap<TypeId, c_int>>>,
                );
                ffi::lua_rawset(state, -3);

                ffi::lua_setmetatable(state, -2);

                ffi::lua_rawset(state, ffi::LUA_REGISTRYINDEX);

                // Create the function metatable

                ffi::lua_pushlightuserdata(
                    state,
                    &FUNCTION_METATABLE_REGISTRY_KEY as *const u8 as *mut c_void,
                );

                ffi::lua_newtable(state);

                push_string(state, "__gc");
                ffi::lua_pushcfunction(state, userdata_destructor::<Callback>);
                ffi::lua_rawset(state, -3);

                push_string(state, "__metatable");
                ffi::lua_pushboolean(state, 0);
                ffi::lua_rawset(state, -3);

                ffi::lua_rawset(state, ffi::LUA_REGISTRYINDEX);

                // Override pcall / xpcall

                ffi::lua_rawgeti(state, ffi::LUA_REGISTRYINDEX, ffi::LUA_RIDX_GLOBALS);

                push_string(state, "pcall");
                ffi::lua_pushcfunction(state, safe_pcall);
                ffi::lua_rawset(state, -3);

                push_string(state, "xpcall");
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
                handle_error(
                    self.state,
                    if let Some(name) = name {
                        let name = CString::new(name.to_owned()).map_err(|e| {
                            Error::ToLuaConversionError(e.to_string())
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

                Ok(Function(self.pop_ref(self.state)))
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
    pub fn create_string(&self, s: &str) -> String {
        unsafe {
            stack_guard(self.state, 0, || {
                check_stack(self.state, 1);
                ffi::lua_pushlstring(self.state, s.as_ptr() as *const c_char, s.len());
                String(self.pop_ref(self.state))
            })
        }
    }

    /// Creates and returns a new table.
    pub fn create_table(&self) -> Table {
        unsafe {
            stack_guard(self.state, 0, || {
                check_stack(self.state, 1);
                ffi::lua_newtable(self.state);
                Table(self.pop_ref(self.state))
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
                check_stack(self.state, 3);
                ffi::lua_newtable(self.state);

                for (k, v) in cont {
                    self.push_value(self.state, k.to_lua(self)?);
                    self.push_value(self.state, v.to_lua(self)?);
                    ffi::lua_rawset(self.state, -3);
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
    /// let greet = lua.create_function(|lua, args| {
    ///     let name: String = lua.unpack(args)?;
    ///     println!("Hello, {}!", name);
    ///     lua.pack(())
    /// });
    /// # let _ = greet;    // used
    /// # Ok(())
    /// # }
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    ///
    /// Use the `hlist_macro` crate to use multiple arguments:
    ///
    /// ```
    /// # extern crate rlua;
    /// # use rlua::{Lua, Result};
    /// # fn try_main() -> Result<()> {
    /// let lua = Lua::new();
    ///
    /// let print_person = lua.create_function(|lua, args| {
    ///     let (name, age): (String, u8) = lua.unpack(args)?;
    ///     println!("{} is {} years old!", name, age);
    ///     lua.pack(())
    /// });
    /// # let _ = print_person;    // used
    /// # Ok(())
    /// # }
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    pub fn create_function<'lua, A, R, F>(&'lua self, mut func: F) -> Function<'lua>
    where
        A: 'lua + FromLuaMulti<'lua>,
        R: 'lua + ToLuaMulti<'lua>,
        F: 'lua + FnMut(&'lua Lua, A) -> Result<R>,
    {
        self.create_callback_function(Box::new(move |lua, args| {
            func(lua, A::from_lua_multi(args, lua)?)?.to_lua_multi(lua)
        }))
    }

    /// Wraps a Lua function into a new thread (or coroutine).
    ///
    /// Equivalent to `coroutine.create`.
    pub fn create_thread<'lua>(&'lua self, func: Function<'lua>) -> Thread<'lua> {
        unsafe {
            stack_guard(self.state, 0, move || {
                check_stack(self.state, 1);

                let thread_state = ffi::lua_newthread(self.state);
                self.push_ref(thread_state, &func.0);

                Thread(self.pop_ref(self.state))
            })
        }
    }

    /// Create a Lua userdata object from a custom userdata type.
    pub fn create_userdata<T>(&self, data: T) -> AnyUserData
    where
        T: UserData,
    {
        unsafe {
            stack_guard(self.state, 0, move || {
                check_stack(self.state, 2);

                push_userdata::<RefCell<T>>(self.state, RefCell::new(data));

                ffi::lua_rawgeti(
                    self.state,
                    ffi::LUA_REGISTRYINDEX,
                    self.userdata_metatable::<T>() as ffi::lua_Integer,
                );

                ffi::lua_setmetatable(self.state, -2);

                AnyUserData(self.pop_ref(self.state))
            })
        }
    }

    /// Returns a handle to the global environment.
    pub fn globals(&self) -> Table {
        unsafe {
            stack_guard(self.state, 0, move || {
                check_stack(self.state, 1);
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
                stack_guard(self.state, 0, || {
                    check_stack(self.state, 1);
                    self.push_value(self.state, v);
                    if ffi::lua_tostring(self.state, -1).is_null() {
                        ffi::lua_pop(self.state, 1);
                        Err(Error::FromLuaConversionError(
                            "cannot convert lua value to string".to_owned(),
                        ))
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
                    self.push_value(self.state, v);
                    let mut isint = 0;
                    let i = ffi::lua_tointegerx(self.state, -1, &mut isint);
                    ffi::lua_pop(self.state, 1);
                    if isint == 0 {
                        Err(Error::FromLuaConversionError(
                            "cannot convert lua value to integer".to_owned(),
                        ))
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
                    self.push_value(self.state, v);
                    let mut isnum = 0;
                    let n = ffi::lua_tonumberx(self.state, -1, &mut isnum);
                    ffi::lua_pop(self.state, 1);
                    if isnum == 0 {
                        Err(Error::FromLuaConversionError(
                            "cannot convert lua value to number".to_owned(),
                        ))
                    } else {
                        Ok(n)
                    }
                })
            },
        }
    }

    pub fn from<'lua, T: ToLua<'lua>>(&'lua self, t: T) -> Result<Value<'lua>> {
        t.to_lua(self)
    }

    pub fn to<'lua, T: FromLua<'lua>>(&'lua self, value: Value<'lua>) -> Result<T> {
        T::from_lua(value, self)
    }

    /// Packs up a value that implements `ToLuaMulti` into a `MultiValue` instance.
    ///
    /// This can be used to return arbitrary Lua values from a Rust function back to Lua.
    pub fn pack<'lua, T: ToLuaMulti<'lua>>(&'lua self, t: T) -> Result<MultiValue<'lua>> {
        t.to_lua_multi(self)
    }

    /// Unpacks a `MultiValue` instance into a value that implements `FromLuaMulti`.
    ///
    /// This can be used to convert the arguments of a Rust function called by Lua.
    pub fn unpack<'lua, T: FromLuaMulti<'lua>>(&'lua self, value: MultiValue<'lua>) -> Result<T> {
        T::from_lua_multi(value, self)
    }

    fn create_callback_function<'lua>(&'lua self, func: Callback<'lua>) -> Function<'lua> {
        unsafe extern "C" fn callback_call_impl(state: *mut ffi::lua_State) -> c_int {
            callback_error(state, || {
                let lua = Lua {
                    state: state,
                    main_state: main_state(state),
                    ephemeral: true,
                };

                let func = &mut *get_userdata::<Callback>(state, ffi::lua_upvalueindex(1));

                let nargs = ffi::lua_gettop(state);
                let mut args = MultiValue::new();
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
                check_stack(self.state, 2);

                push_userdata::<Callback>(self.state, func);

                ffi::lua_pushlightuserdata(
                    self.state,
                    &FUNCTION_METATABLE_REGISTRY_KEY as *const u8 as *mut c_void,
                );
                ffi::lua_gettable(self.state, ffi::LUA_REGISTRYINDEX);
                ffi::lua_setmetatable(self.state, -2);

                ffi::lua_pushcclosure(self.state, callback_call_impl, 1);

                Function(self.pop_ref(self.state))
            })
        }
    }

    unsafe fn push_value(&self, state: *mut ffi::lua_State, value: Value) {
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

    unsafe fn pop_value(&self, state: *mut ffi::lua_State) -> Value {
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

            ffi::LUA_TNUMBER => {
                if ffi::lua_isinteger(state, -1) != 0 {
                    let i = Value::Integer(ffi::lua_tointeger(state, -1));
                    ffi::lua_pop(state, 1);
                    i
                } else {
                    let n = Value::Number(ffi::lua_tonumber(state, -1));
                    ffi::lua_pop(state, 1);
                    n
                }
            }

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

    unsafe fn push_ref(&self, state: *mut ffi::lua_State, lref: &LuaRef) {
        assert_eq!(
            lref.lua.main_state,
            self.main_state,
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
    unsafe fn pop_ref(&self, state: *mut ffi::lua_State) -> LuaRef {
        let registry_id = ffi::luaL_ref(state, ffi::LUA_REGISTRYINDEX);
        LuaRef {
            lua: self,
            registry_id: registry_id,
        }
    }

    unsafe fn userdata_metatable<T: UserData>(&self) -> c_int {
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
            check_stack(self.state, 5);

            ffi::lua_pushlightuserdata(
                self.state,
                &LUA_USERDATA_REGISTRY_KEY as *const u8 as *mut c_void,
            );
            ffi::lua_gettable(self.state, ffi::LUA_REGISTRYINDEX);
            let registered_userdata =
                &mut *get_userdata::<RefCell<HashMap<TypeId, c_int>>>(self.state, -1);
            let mut map = (*registered_userdata).borrow_mut();
            ffi::lua_pop(self.state, 1);

            match map.entry(TypeId::of::<T>()) {
                HashMapEntry::Occupied(entry) => *entry.get(),
                HashMapEntry::Vacant(entry) => {
                    ffi::lua_newtable(self.state);

                    let mut methods = UserDataMethods {
                        methods: HashMap::new(),
                        meta_methods: HashMap::new(),
                        _type: PhantomData,
                    };
                    T::add_methods(&mut methods);

                    let has_methods = !methods.methods.is_empty();

                    if has_methods {
                        push_string(self.state, "__index");
                        ffi::lua_newtable(self.state);

                        for (k, m) in methods.methods {
                            push_string(self.state, &k);
                            self.push_value(
                                self.state,
                                Value::Function(self.create_callback_function(m)),
                            );
                            ffi::lua_rawset(self.state, -3);
                        }

                        ffi::lua_rawset(self.state, -3);
                    }

                    for (k, m) in methods.meta_methods {
                        if k == MetaMethod::Index && has_methods {
                            push_string(self.state, "__index");
                            ffi::lua_pushvalue(self.state, -1);
                            ffi::lua_gettable(self.state, -3);
                            self.push_value(
                                self.state,
                                Value::Function(self.create_callback_function(m)),
                            );
                            ffi::lua_pushcclosure(self.state, meta_index_impl, 2);
                            ffi::lua_rawset(self.state, -3);
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
                            push_string(self.state, name);
                            self.push_value(
                                self.state,
                                Value::Function(self.create_callback_function(m)),
                            );
                            ffi::lua_rawset(self.state, -3);
                        }
                    }

                    push_string(self.state, "__gc");
                    ffi::lua_pushcfunction(self.state, userdata_destructor::<RefCell<T>>);
                    ffi::lua_rawset(self.state, -3);

                    push_string(self.state, "__metatable");
                    ffi::lua_pushboolean(self.state, 0);
                    ffi::lua_rawset(self.state, -3);

                    let id = ffi::luaL_ref(self.state, ffi::LUA_REGISTRYINDEX);
                    entry.insert(id);
                    id
                }
            }
        })
    }
}

static LUA_USERDATA_REGISTRY_KEY: u8 = 0;
static FUNCTION_METATABLE_REGISTRY_KEY: u8 = 0;
