use std::iter::{self, FromIterator};
use std::{slice, str, vec};

use crate::context::Context;
use crate::error::{Error, Result};
use crate::function::Function;
use crate::string::String;
use crate::table::Table;
use crate::thread::Thread;
#[cfg(any(rlua_lua53, rlua_lua54))]
use crate::types::Integer;
use crate::types::{LightUserData, Number};
use crate::userdata::AnyUserData;

/// A dynamically typed Lua value.  The `String`, `Table`, `Function`, `Thread`, and `UserData`
/// variants contain handle types into the internal Lua state.  It is a logic error to mix handle
/// types between separate `Lua` instances, or between a parent `Lua` instance and one received as a
/// parameter in a Rust callback, and doing so will result in a panic.
#[derive(Debug, Clone)]
pub enum Value<'lua> {
    /// The Lua value `nil`.
    Nil,
    /// The Lua value `true` or `false`.
    Boolean(bool),
    /// A "light userdata" object, equivalent to a raw pointer.
    LightUserData(LightUserData),
    #[cfg(any(rlua_lua53, rlua_lua54))]
    /// An integer number.
    ///
    /// Any Lua number convertible to a `Integer` will be represented as this variant.
    /// (Lua 5.3+ only)
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
    /// Reference to a userdata object that holds a custom type which implements `UserData`.
    /// Special builtin userdata types will be represented as other `Value` variants.
    UserData(AnyUserData<'lua>),
    /// `Error` is a special builtin userdata type.  When received from Lua it is implicitly cloned.
    Error(Error),
}
pub use self::Value::Nil;

impl<'lua> Value<'lua> {
    pub fn type_name(&self) -> &'static str {
        match *self {
            Value::Nil => "nil",
            Value::Boolean(_) => "boolean",
            Value::LightUserData(_) => "lightuserdata",
            #[cfg(any(rlua_lua53, rlua_lua54))]
            Value::Integer(_) => "integer",
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Table(_) => "table",
            Value::Function(_) => "function",
            Value::Thread(_) => "thread",
            Value::UserData(_) => "userdata",
            Value::Error(_) => "error",
        }
    }
}

/// Trait for types convertible to `Value`.
pub trait ToLua<'lua> {
    /// Performs the conversion.
    fn to_lua(self, lua: Context<'lua>) -> Result<Value<'lua>>;
}

/// Trait for types convertible from `Value`.
pub trait FromLua<'lua>: Sized {
    /// Performs the conversion.
    fn from_lua(lua_value: Value<'lua>, lua: Context<'lua>) -> Result<Self>;
}

/// Multiple Lua values used for both argument passing and also for multiple return values.
#[derive(Debug, Clone)]
pub struct MultiValue<'lua>(pub(crate) Vec<Value<'lua>>);

impl<'lua> MultiValue<'lua> {
    /// Creates an empty `MultiValue` containing no values.
    pub fn new() -> MultiValue<'lua> {
        MultiValue(Vec::new())
    }
}

impl<'lua> Default for MultiValue<'lua> {
    fn default() -> MultiValue<'lua> {
        MultiValue::new()
    }
}

impl<'lua> FromIterator<Value<'lua>> for MultiValue<'lua> {
    fn from_iter<I: IntoIterator<Item = Value<'lua>>>(iter: I) -> Self {
        MultiValue::from_vec(Vec::from_iter(iter))
    }
}

impl<'lua> IntoIterator for MultiValue<'lua> {
    type Item = Value<'lua>;
    type IntoIter = iter::Rev<vec::IntoIter<Value<'lua>>>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter().rev()
    }
}

impl<'a, 'lua> IntoIterator for &'a MultiValue<'lua> {
    type Item = &'a Value<'lua>;
    type IntoIter = iter::Rev<slice::Iter<'a, Value<'lua>>>;

    fn into_iter(self) -> Self::IntoIter {
        (&self.0).into_iter().rev()
    }
}

impl<'lua> MultiValue<'lua> {
    pub fn from_vec(mut v: Vec<Value<'lua>>) -> MultiValue<'lua> {
        v.reverse();
        MultiValue(v)
    }

    pub fn into_vec(self) -> Vec<Value<'lua>> {
        let mut v = self.0;
        v.reverse();
        v
    }

    pub(crate) fn reserve(&mut self, size: usize) {
        self.0.reserve(size);
    }

    pub(crate) fn push_front(&mut self, value: Value<'lua>) {
        self.0.push(value);
    }

    pub(crate) fn push_front_many(&mut self, values: Self) {
        self.0.extend(values.into_iter());
    }

    pub(crate) fn pop_front(&mut self) -> Option<Value<'lua>> {
        self.0.pop()
    }

    pub(crate) fn drop_front(&mut self, count: usize) {
        self.0.drain(self.0.len() - count..).count();
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.len() == 0
    }

    pub fn iter(&self) -> iter::Rev<slice::Iter<Value<'lua>>> {
        self.0.iter().rev()
    }
}

/// Trait for types convertible to any number of Lua values.
///
/// This is a generalization of `ToLua`, allowing any number of resulting Lua values instead of just
/// one. Any type that implements `ToLua` will automatically implement this trait.
pub trait ToLuaMulti<'lua> {
    /// Performs the conversion.
    fn to_lua_multi(self, lua: Context<'lua>) -> Result<MultiValue<'lua>>;
}

/// Trait for types that can be created from an arbitrary number of Lua values.
///
/// This is a generalization of `FromLua`, allowing an arbitrary number of Lua values to participate
/// in the conversion. Any type that implements `FromLua` will automatically implement this trait.
pub trait FromLuaMulti<'lua>: Sized {
    /// Performs the conversion.
    ///
    /// In case `values` contains more values than needed to perform the conversion, the excess
    /// values should be ignored. This reflects the semantics of Lua when calling a function or
    /// assigning values. Similarly, if not enough values are given, conversions should assume that
    /// any missing values are nil.
    ///
    /// The `consumed` reference should be updated accordingly to notify the outer scope
    /// about the number of values that were used to perform the conversion. Failing to
    /// do so will make the outer scope try and use the same sequence of `values` for
    /// multiple consecutive conversions.
    /// In case of an error, the `consumed` counter value should be kept the same to allow
    /// wrappers such as [`Variadic`] to conservatively capture values.
    ///
    /// [`Variadic`]: struct.Variadic.html
    fn from_lua_multi(
        values: MultiValue<'lua>,
        lua: Context<'lua>,
        consumed: &mut usize,
    ) -> Result<Self>;
}
