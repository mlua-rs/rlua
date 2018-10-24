use std::collections::{BTreeMap, HashMap};
use std::hash::{BuildHasher, Hash};
use std::string::String as StdString;

use num_traits::cast;

use error::{Error, Result};
use function::Function;
use lua::Lua;
use string::String;
use table::Table;
use thread::Thread;
use types::{LightUserData, Number};
use userdata::{AnyUserData, UserData};
use value::{FromLua, Nil, ToLua, Value};

impl<'a> ToLua for Value<'a> {
    fn to_lua<'lua>(self, lua: &'lua Lua) -> Result<Value<'lua>> {
        Ok(match self {
            Value::Nil => Nil,
            Value::Boolean(b) => Value::Boolean(b),
            Value::LightUserData(l) => Value::LightUserData(l),
            Value::Integer(i) => Value::Integer(i),
            Value::Number(n) => Value::Number(n),
            Value::String(s) => Value::String(String(lua.adopt_ref(s.0))),
            Value::Table(s) => Value::Table(Table(lua.adopt_ref(s.0))),
            Value::Function(s) => Value::Function(Function(lua.adopt_ref(s.0))),
            Value::Thread(s) => Value::Thread(Thread(lua.adopt_ref(s.0))),
            Value::UserData(s) => Value::UserData(AnyUserData(lua.adopt_ref(s.0))),
            Value::Error(e) => Value::Error(e),
        })
    }
}

impl<'lua> FromLua<'lua> for Value<'lua> {
    fn from_lua(lua_value: Value<'lua>, _: &'lua Lua) -> Result<Self> {
        Ok(lua_value)
    }
}

impl<'a> ToLua for String<'a> {
    fn to_lua<'lua>(self, lua: &'lua Lua) -> Result<Value<'lua>> {
        Ok(Value::String(String(lua.adopt_ref(self.0))))
    }
}

impl<'lua> FromLua<'lua> for String<'lua> {
    fn from_lua(value: Value<'lua>, lua: &'lua Lua) -> Result<String<'lua>> {
        let ty = value.type_name();
        lua.coerce_string(value)
            .ok_or_else(|| Error::FromLuaConversionError {
                from: ty,
                to: "String",
                message: Some("expected string or number".to_string()),
            })
    }
}

impl<'a> ToLua for Table<'a> {
    fn to_lua<'lua>(self, lua: &'lua Lua) -> Result<Value<'lua>> {
        Ok(Value::Table(Table(lua.adopt_ref(self.0))))
    }
}

impl<'lua> FromLua<'lua> for Table<'lua> {
    fn from_lua(value: Value<'lua>, _: &'lua Lua) -> Result<Table<'lua>> {
        match value {
            Value::Table(table) => Ok(table),
            _ => Err(Error::FromLuaConversionError {
                from: value.type_name(),
                to: "table",
                message: None,
            }),
        }
    }
}

impl<'a> ToLua for Function<'a> {
    fn to_lua<'lua>(self, lua: &'lua Lua) -> Result<Value<'lua>> {
        Ok(Value::Function(Function(lua.adopt_ref(self.0))))
    }
}

impl<'lua> FromLua<'lua> for Function<'lua> {
    fn from_lua(value: Value<'lua>, _: &'lua Lua) -> Result<Function<'lua>> {
        match value {
            Value::Function(table) => Ok(table),
            _ => Err(Error::FromLuaConversionError {
                from: value.type_name(),
                to: "function",
                message: None,
            }),
        }
    }
}

impl<'a> ToLua for Thread<'a> {
    fn to_lua<'lua>(self, lua: &'lua Lua) -> Result<Value<'lua>> {
        Ok(Value::Thread(Thread(lua.adopt_ref(self.0))))
    }
}

impl<'lua> FromLua<'lua> for Thread<'lua> {
    fn from_lua(value: Value<'lua>, _: &'lua Lua) -> Result<Thread<'lua>> {
        match value {
            Value::Thread(t) => Ok(t),
            _ => Err(Error::FromLuaConversionError {
                from: value.type_name(),
                to: "thread",
                message: None,
            }),
        }
    }
}

impl<'a> ToLua for AnyUserData<'a> {
    fn to_lua<'lua>(self, lua: &'lua Lua) -> Result<Value<'lua>> {
        Ok(Value::UserData(AnyUserData(lua.adopt_ref(self.0))))
    }
}

impl<'lua> FromLua<'lua> for AnyUserData<'lua> {
    fn from_lua(value: Value<'lua>, _: &'lua Lua) -> Result<AnyUserData<'lua>> {
        match value {
            Value::UserData(ud) => Ok(ud),
            _ => Err(Error::FromLuaConversionError {
                from: value.type_name(),
                to: "userdata",
                message: None,
            }),
        }
    }
}

impl<T: 'static + Send + UserData> ToLua for T {
    fn to_lua<'lua>(self, lua: &'lua Lua) -> Result<Value<'lua>> {
        Ok(Value::UserData(lua.create_userdata(self)?))
    }
}

impl<'lua, T: 'static + UserData + Clone> FromLua<'lua> for T {
    fn from_lua(value: Value<'lua>, _: &'lua Lua) -> Result<T> {
        match value {
            Value::UserData(ud) => Ok(ud.borrow::<T>()?.clone()),
            _ => Err(Error::FromLuaConversionError {
                from: value.type_name(),
                to: "userdata",
                message: None,
            }),
        }
    }
}

impl ToLua for Error {
    fn to_lua<'lua>(self, _: &'lua Lua) -> Result<Value<'lua>> {
        Ok(Value::Error(self))
    }
}

impl<'lua> FromLua<'lua> for Error {
    fn from_lua(value: Value<'lua>, lua: &'lua Lua) -> Result<Error> {
        match value {
            Value::Error(err) => Ok(err),
            val => Ok(Error::RuntimeError(
                lua.coerce_string(val)
                    .and_then(|s| Some(s.to_str().ok()?.to_owned()))
                    .unwrap_or_else(|| "<unprintable error>".to_owned()),
            )),
        }
    }
}

impl ToLua for bool {
    fn to_lua<'lua>(self, _: &'lua Lua) -> Result<Value<'lua>> {
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

impl ToLua for LightUserData {
    fn to_lua<'lua>(self, _: &'lua Lua) -> Result<Value<'lua>> {
        Ok(Value::LightUserData(self))
    }
}

impl<'lua> FromLua<'lua> for LightUserData {
    fn from_lua(value: Value, _: &'lua Lua) -> Result<Self> {
        match value {
            Value::LightUserData(ud) => Ok(ud),
            _ => Err(Error::FromLuaConversionError {
                from: value.type_name(),
                to: "light userdata",
                message: None,
            }),
        }
    }
}

impl ToLua for StdString {
    fn to_lua<'lua>(self, lua: &'lua Lua) -> Result<Value<'lua>> {
        Ok(Value::String(lua.create_string(&self)?))
    }
}

impl<'lua> FromLua<'lua> for StdString {
    fn from_lua(value: Value<'lua>, lua: &'lua Lua) -> Result<Self> {
        let ty = value.type_name();
        Ok(lua
            .coerce_string(value)
            .ok_or_else(|| Error::FromLuaConversionError {
                from: ty,
                to: "String",
                message: Some("expected string or number".to_string()),
            })?.to_str()?
            .to_owned())
    }
}

impl<'a> ToLua for &'a str {
    fn to_lua<'lua>(self, lua: &'lua Lua) -> Result<Value<'lua>> {
        Ok(Value::String(lua.create_string(self)?))
    }
}

macro_rules! lua_convert_int {
    ($x:ty) => {
        impl ToLua for $x {
            fn to_lua<'lua>(self, _: &'lua Lua) -> Result<Value<'lua>> {
                if let Some(i) = cast(self) {
                    Ok(Value::Integer(i))
                } else {
                    cast(self)
                        .ok_or_else(|| Error::ToLuaConversionError {
                            from: stringify!($x),
                            to: "number",
                            message: Some("out of range".to_owned()),
                        }).map(Value::Number)
                }
            }
        }

        impl<'lua> FromLua<'lua> for $x {
            fn from_lua(value: Value<'lua>, lua: &'lua Lua) -> Result<Self> {
                let ty = value.type_name();
                (if let Some(i) = lua.coerce_integer(value.clone()) {
                    cast(i)
                } else {
                    cast(
                        lua.coerce_number(value)
                            .ok_or_else(|| Error::FromLuaConversionError {
                                from: ty,
                                to: stringify!($x),
                                message: Some(
                                    "expected number or string coercible to number".to_string(),
                                ),
                            })?,
                    )
                }).ok_or_else(|| Error::FromLuaConversionError {
                    from: ty,
                    to: stringify!($x),
                    message: Some("out of range".to_owned()),
                })
            }
        }
    };
}

lua_convert_int!(i8);
lua_convert_int!(u8);
lua_convert_int!(i16);
lua_convert_int!(u16);
lua_convert_int!(i32);
lua_convert_int!(u32);
lua_convert_int!(i64);
lua_convert_int!(u64);
lua_convert_int!(i128);
lua_convert_int!(u128);
lua_convert_int!(isize);
lua_convert_int!(usize);

macro_rules! lua_convert_float {
    ($x:ty) => {
        impl ToLua for $x {
            fn to_lua<'lua>(self, _: &'lua Lua) -> Result<Value<'lua>> {
                Ok(Value::Number(self as Number))
            }
        }

        impl<'lua> FromLua<'lua> for $x {
            fn from_lua(value: Value<'lua>, lua: &'lua Lua) -> Result<Self> {
                let ty = value.type_name();
                lua.coerce_number(value)
                    .ok_or_else(|| Error::FromLuaConversionError {
                        from: ty,
                        to: stringify!($x),
                        message: Some("expected number or string coercible to number".to_string()),
                    }).and_then(|n| {
                        cast(n).ok_or_else(|| Error::FromLuaConversionError {
                            from: ty,
                            to: stringify!($x),
                            message: Some("number out of range".to_string()),
                        })
                    })
            }
        }
    };
}

lua_convert_float!(f32);
lua_convert_float!(f64);

impl<T: ToLua> ToLua for Vec<T> {
    fn to_lua<'lua>(self, lua: &'lua Lua) -> Result<Value<'lua>> {
        Ok(Value::Table(lua.create_sequence_from(self)?))
    }
}

impl<'lua, T: FromLua<'lua>> FromLua<'lua> for Vec<T> {
    fn from_lua(value: Value<'lua>, _: &'lua Lua) -> Result<Self> {
        if let Value::Table(table) = value {
            table.sequence_values().collect()
        } else {
            Err(Error::FromLuaConversionError {
                from: value.type_name(),
                to: "Vec",
                message: Some("expected table".to_string()),
            })
        }
    }
}

impl<K: Eq + Hash + ToLua, V: ToLua, S: BuildHasher> ToLua for HashMap<K, V, S> {
    fn to_lua<'lua>(self, lua: &'lua Lua) -> Result<Value<'lua>> {
        Ok(Value::Table(lua.create_table_from(self)?))
    }
}

impl<'lua, K: Eq + Hash + FromLua<'lua>, V: FromLua<'lua>, S: BuildHasher + Default> FromLua<'lua>
    for HashMap<K, V, S>
{
    fn from_lua(value: Value<'lua>, _: &'lua Lua) -> Result<Self> {
        if let Value::Table(table) = value {
            table.pairs().collect()
        } else {
            Err(Error::FromLuaConversionError {
                from: value.type_name(),
                to: "HashMap",
                message: Some("expected table".to_string()),
            })
        }
    }
}

impl<K: Ord + ToLua, V: ToLua> ToLua for BTreeMap<K, V> {
    fn to_lua<'lua>(self, lua: &'lua Lua) -> Result<Value<'lua>> {
        Ok(Value::Table(lua.create_table_from(self)?))
    }
}

impl<'lua, K: Ord + FromLua<'lua>, V: FromLua<'lua>> FromLua<'lua> for BTreeMap<K, V> {
    fn from_lua(value: Value<'lua>, _: &'lua Lua) -> Result<Self> {
        if let Value::Table(table) = value {
            table.pairs().collect()
        } else {
            Err(Error::FromLuaConversionError {
                from: value.type_name(),
                to: "BTreeMap",
                message: Some("expected table".to_string()),
            })
        }
    }
}

impl<T: ToLua> ToLua for Option<T> {
    fn to_lua<'lua>(self, lua: &'lua Lua) -> Result<Value<'lua>> {
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
