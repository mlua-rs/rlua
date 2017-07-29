use std::collections::{HashMap, BTreeMap};
use std::hash::Hash;
use std::string::String as StdString;

use error::*;
use lua::*;

impl<'lua> ToLua<'lua> for Value<'lua> {
    fn to_lua(self, _: &'lua Lua) -> Result<Value<'lua>> {
        Ok(self)
    }
}

impl<'lua> FromLua<'lua> for Value<'lua> {
    fn from_lua(lua_value: Value<'lua>, _: &'lua Lua) -> Result<Self> {
        Ok(lua_value)
    }
}

impl<'lua> ToLua<'lua> for String<'lua> {
    fn to_lua(self, _: &'lua Lua) -> Result<Value<'lua>> {
        Ok(Value::String(self))
    }
}

impl<'lua> FromLua<'lua> for String<'lua> {
    fn from_lua(value: Value<'lua>, lua: &'lua Lua) -> Result<String<'lua>> {
        lua.coerce_string(value)
    }
}

impl<'lua> ToLua<'lua> for Table<'lua> {
    fn to_lua(self, _: &'lua Lua) -> Result<Value> {
        Ok(Value::Table(self))
    }
}

impl<'lua> FromLua<'lua> for Table<'lua> {
    fn from_lua(value: Value<'lua>, _: &'lua Lua) -> Result<Table<'lua>> {
        match value {
            Value::Table(table) => Ok(table),
            _ => Err(Error::FromLuaConversionError(
                "cannot convert lua value to table".to_owned(),
            )),
        }
    }
}

impl<'lua> ToLua<'lua> for Function<'lua> {
    fn to_lua(self, _: &'lua Lua) -> Result<Value<'lua>> {
        Ok(Value::Function(self))
    }
}

impl<'lua> FromLua<'lua> for Function<'lua> {
    fn from_lua(value: Value<'lua>, _: &'lua Lua) -> Result<Function<'lua>> {
        match value {
            Value::Function(table) => Ok(table),
            _ => Err(Error::FromLuaConversionError(
                "cannot convert lua value to function".to_owned(),
            )),
        }
    }
}

impl<'lua> ToLua<'lua> for Thread<'lua> {
    fn to_lua(self, _: &'lua Lua) -> Result<Value<'lua>> {
        Ok(Value::Thread(self))
    }
}

impl<'lua> FromLua<'lua> for Thread<'lua> {
    fn from_lua(value: Value<'lua>, _: &'lua Lua) -> Result<Thread<'lua>> {
        match value {
            Value::Thread(t) => Ok(t),
            _ => Err(Error::FromLuaConversionError(
                "cannot convert lua value to thread".to_owned(),
            )),
        }
    }
}

impl<'lua> ToLua<'lua> for AnyUserData<'lua> {
    fn to_lua(self, _: &'lua Lua) -> Result<Value<'lua>> {
        Ok(Value::UserData(self))
    }
}

impl<'lua> FromLua<'lua> for AnyUserData<'lua> {
    fn from_lua(value: Value<'lua>, _: &'lua Lua) -> Result<AnyUserData<'lua>> {
        match value {
            Value::UserData(ud) => Ok(ud),
            _ => Err(Error::FromLuaConversionError(
                "cannot convert lua value to userdata".to_owned(),
            )),
        }
    }
}

impl<'lua, T: UserData> ToLua<'lua> for T {
    fn to_lua(self, lua: &'lua Lua) -> Result<Value<'lua>> {
        Ok(Value::UserData(lua.create_userdata(self)))
    }
}

impl<'lua, T: UserData + Clone> FromLua<'lua> for T {
    fn from_lua(value: Value<'lua>, _: &'lua Lua) -> Result<T> {
        match value {
            Value::UserData(ud) => Ok(ud.borrow::<T>()?.clone()),
            _ => Err(Error::FromLuaConversionError(
                "cannot convert lua value to userdata".to_owned(),
            )),
        }
    }
}

impl<'lua> ToLua<'lua> for Error {
    fn to_lua(self, _: &'lua Lua) -> Result<Value<'lua>> {
        Ok(Value::Error(self))
    }
}

impl<'lua> FromLua<'lua> for Error {
    fn from_lua(value: Value<'lua>, lua: &'lua Lua) -> Result<Error> {
        match value {
            Value::Error(err) => Ok(err),
            val => Ok(Error::RuntimeError(
                lua.coerce_string(val)
                    .and_then(|s| Ok(s.to_str()?.to_owned()))
                    .unwrap_or_else(|_| "<unprintable error>".to_owned()),
            )),
        }
    }
}

impl<'lua> ToLua<'lua> for bool {
    fn to_lua(self, _: &'lua Lua) -> Result<Value<'lua>> {
        Ok(Value::Boolean(self))
    }
}

impl<'lua> FromLua<'lua> for bool {
    fn from_lua(v: Value, _: &'lua Lua) -> Result<Self> {
        match v {
            Value::Nil => Ok(false),
            Value::Boolean(b) => Ok(b),
            _ => Ok(true),
        }
    }
}

impl<'lua> ToLua<'lua> for LightUserData {
    fn to_lua(self, _: &'lua Lua) -> Result<Value<'lua>> {
        Ok(Value::LightUserData(self))
    }
}

impl<'lua> FromLua<'lua> for LightUserData {
    fn from_lua(v: Value, _: &'lua Lua) -> Result<Self> {
        match v {
            Value::LightUserData(ud) => Ok(ud),
            _ => Err(Error::FromLuaConversionError(
                "cannot convert lua value to lightuserdata".to_owned(),
            )),
        }
    }
}

impl<'lua> ToLua<'lua> for StdString {
    fn to_lua(self, lua: &'lua Lua) -> Result<Value<'lua>> {
        Ok(Value::String(lua.create_string(&self)))
    }
}

impl<'lua> FromLua<'lua> for StdString {
    fn from_lua(value: Value<'lua>, lua: &'lua Lua) -> Result<Self> {
        Ok(lua.coerce_string(value)?.to_str()?.to_owned())
    }
}

impl<'lua, 'a> ToLua<'lua> for &'a str {
    fn to_lua(self, lua: &'lua Lua) -> Result<Value<'lua>> {
        Ok(Value::String(lua.create_string(self)))
    }
}

macro_rules! lua_convert_int {
    ($x: ty) => {
        impl<'lua> ToLua<'lua> for $x {
            fn to_lua(self, _: &'lua Lua) -> Result<Value<'lua>> {
                Ok(Value::Integer(self as Integer))
            }
        }

        impl<'lua> FromLua<'lua> for $x {
            fn from_lua(value: Value<'lua>, lua: &'lua Lua) -> Result<Self> {
                Ok(lua.coerce_integer(value)? as $x)
            }
        }
    }
}

lua_convert_int!(i8);
lua_convert_int!(u8);
lua_convert_int!(i16);
lua_convert_int!(u16);
lua_convert_int!(i32);
lua_convert_int!(u32);
lua_convert_int!(i64);
lua_convert_int!(u64);
lua_convert_int!(isize);
lua_convert_int!(usize);

macro_rules! lua_convert_float {
    ($x: ty) => {
        impl<'lua> ToLua<'lua> for $x {
            fn to_lua(self, _: &'lua Lua) -> Result<Value<'lua>> {
                Ok(Value::Number(self as Number))
            }
        }

        impl<'lua> FromLua<'lua> for $x {
            fn from_lua(value: Value<'lua>, lua: &'lua Lua) -> Result<Self> {
                Ok(lua.coerce_number(value)? as $x)
            }
        }
    }
}

lua_convert_float!(f32);
lua_convert_float!(f64);

impl<'lua, T: ToLua<'lua>> ToLua<'lua> for Vec<T> {
    fn to_lua(self, lua: &'lua Lua) -> Result<Value<'lua>> {
        Ok(Value::Table(lua.create_sequence_from(self)?))
    }
}

impl<'lua, T: FromLua<'lua>> FromLua<'lua> for Vec<T> {
    fn from_lua(value: Value<'lua>, _: &'lua Lua) -> Result<Self> {
        if let Value::Table(table) = value {
            table.sequence_values().collect()
        } else {
            Err(Error::FromLuaConversionError(
                "cannot convert lua value to table for Vec".to_owned(),
            ))
        }
    }
}

impl<'lua, K: Eq + Hash + ToLua<'lua>, V: ToLua<'lua>> ToLua<'lua> for HashMap<K, V> {
    fn to_lua(self, lua: &'lua Lua) -> Result<Value<'lua>> {
        Ok(Value::Table(lua.create_table_from(self)?))
    }
}

impl<'lua, K: Eq + Hash + FromLua<'lua>, V: FromLua<'lua>> FromLua<'lua> for HashMap<K, V> {
    fn from_lua(value: Value<'lua>, _: &'lua Lua) -> Result<Self> {
        if let Value::Table(table) = value {
            table.pairs().collect()
        } else {
            Err(Error::FromLuaConversionError(
                "cannot convert lua value to table for HashMap".to_owned(),
            ))
        }
    }
}

impl<'lua, K: Ord + ToLua<'lua>, V: ToLua<'lua>> ToLua<'lua> for BTreeMap<K, V> {
    fn to_lua(self, lua: &'lua Lua) -> Result<Value<'lua>> {
        Ok(Value::Table(lua.create_table_from(self)?))
    }
}

impl<'lua, K: Ord + FromLua<'lua>, V: FromLua<'lua>> FromLua<'lua> for BTreeMap<K, V> {
    fn from_lua(value: Value<'lua>, _: &'lua Lua) -> Result<Self> {
        if let Value::Table(table) = value {
            table.pairs().collect()
        } else {
            Err(Error::FromLuaConversionError(
                "cannot convert lua value to table for BTreeMap".to_owned(),
            ))
        }
    }
}

impl<'lua, T: ToLua<'lua>> ToLua<'lua> for Option<T> {
    fn to_lua(self, lua: &'lua Lua) -> Result<Value<'lua>> {
        match self {
            Some(val) => val.to_lua(lua),
            None => Ok(Nil),
        }
    }
}

impl<'lua, T: FromLua<'lua>> FromLua<'lua> for Option<T> {
    fn from_lua(value: Value<'lua>, lua: &'lua Lua) -> Result<Self> {
        match value {
            Nil => Ok(None),
            value => Ok(Some(T::from_lua(value, lua)?)),
        }
    }
}
