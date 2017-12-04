use std::str;
use std::ops::{Deref, DerefMut};
use std::iter::FromIterator;
use std::collections::VecDeque;

use error::*;
use types::{Integer, LightUserData, Number};
use string::String;
use table::Table;
use function::Function;
use thread::Thread;
use userdata::AnyUserData;
use lua::Lua;

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
    /// Reference to a userdata object that holds a custom type which implements `UserData`.
    /// Special builtin userdata types will be represented as other `Value` variants.
    UserData(AnyUserData<'lua>),
    /// `Error` is a special builtin userdata type.  When received from Lua it is implicitly cloned.
    Error(Error),
}
pub use self::Value::Nil;

impl<'lua> Value<'lua> {
    pub(crate) fn type_name(&self) -> &'static str {
        match *self {
            Value::Nil => "nil",
            Value::Boolean(_) => "boolean",
            Value::LightUserData(_) => "light userdata",
            Value::Integer(_) => "integer",
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Table(_) => "table",
            Value::Function(_) => "function",
            Value::Thread(_) => "thread",
            Value::UserData(_) | Value::Error(_) => "userdata",
        }
    }
}

/// Trait for types convertible to `Value`.
pub trait ToLua<'lua> {
    /// Performs the conversion.
    fn to_lua(self, lua: &'lua Lua) -> Result<Value<'lua>>;
}

/// Trait for types convertible from `Value`.
pub trait FromLua<'lua>: Sized {
    /// Performs the conversion.
    fn from_lua(lua_value: Value<'lua>, lua: &'lua Lua) -> Result<Self>;
}

/// Multiple Lua values used for both argument passing and also for multiple return values.
#[derive(Debug, Clone)]
pub struct MultiValue<'lua>(VecDeque<Value<'lua>>);

impl<'lua> MultiValue<'lua> {
    /// Creates an empty `MultiValue` containing no values.
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
pub trait ToLuaMulti<'lua> {
    /// Performs the conversion.
    fn to_lua_multi(self, lua: &'lua Lua) -> Result<MultiValue<'lua>>;
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
    fn from_lua_multi(values: MultiValue<'lua>, lua: &'lua Lua) -> Result<Self>;
}
