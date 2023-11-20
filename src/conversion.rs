use std::collections::{BTreeMap, HashMap};
use std::convert::TryInto;
use std::ffi::{CStr, CString};
use std::hash::{BuildHasher, Hash};
use std::string::String as StdString;

use bstr::{BStr, BString};
use num_traits::cast;

use crate::context::Context;
use crate::error::{Error, Result};
use crate::function::Function;
use crate::string::String;
use crate::table::Table;
use crate::thread::Thread;
use crate::types::{LightUserData, Number};
use crate::userdata::{AnyUserData, UserData};
use crate::value::{FromLua, Nil, ToLua, Value};
use crate::{FromLuaMulti, MultiValue};

impl<'lua> ToLua<'lua> for Value<'lua> {
    fn to_lua(self, _: Context<'lua>) -> Result<Value<'lua>> {
        Ok(self)
    }
}

impl<'lua> FromLua<'lua> for Value<'lua> {
    fn from_lua(lua_value: Value<'lua>, _: Context<'lua>) -> Result<Self> {
        Ok(lua_value)
    }
}

impl<'lua> ToLua<'lua> for String<'lua> {
    fn to_lua(self, _: Context<'lua>) -> Result<Value<'lua>> {
        Ok(Value::String(self))
    }
}

impl<'lua> FromLua<'lua> for String<'lua> {
    fn from_lua(value: Value<'lua>, lua: Context<'lua>) -> Result<String<'lua>> {
        let ty = value.type_name();
        lua.coerce_string(value)?
            .ok_or_else(|| Error::FromLuaConversionError {
                from: ty,
                to: "String",
                message: Some("expected string or number".to_string()),
            })
    }
}

impl<'lua> ToLua<'lua> for Table<'lua> {
    fn to_lua(self, _: Context<'lua>) -> Result<Value<'lua>> {
        Ok(Value::Table(self))
    }
}

impl<'lua> FromLua<'lua> for Table<'lua> {
    fn from_lua(value: Value<'lua>, _: Context<'lua>) -> Result<Table<'lua>> {
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

impl<'lua> ToLua<'lua> for Function<'lua> {
    fn to_lua(self, _: Context<'lua>) -> Result<Value<'lua>> {
        Ok(Value::Function(self))
    }
}

impl<'lua> FromLua<'lua> for Function<'lua> {
    fn from_lua(value: Value<'lua>, _: Context<'lua>) -> Result<Function<'lua>> {
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

impl<'lua> ToLua<'lua> for Thread<'lua> {
    fn to_lua(self, _: Context<'lua>) -> Result<Value<'lua>> {
        Ok(Value::Thread(self))
    }
}

impl<'lua> FromLua<'lua> for Thread<'lua> {
    fn from_lua(value: Value<'lua>, _: Context<'lua>) -> Result<Thread<'lua>> {
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

impl<'lua> ToLua<'lua> for AnyUserData<'lua> {
    fn to_lua(self, _: Context<'lua>) -> Result<Value<'lua>> {
        Ok(Value::UserData(self))
    }
}

impl<'lua> FromLua<'lua> for AnyUserData<'lua> {
    fn from_lua(value: Value<'lua>, _: Context<'lua>) -> Result<AnyUserData<'lua>> {
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

impl<'lua, T: 'static + Send + UserData> ToLua<'lua> for T {
    fn to_lua(self, lua: Context<'lua>) -> Result<Value<'lua>> {
        Ok(Value::UserData(lua.create_userdata(self)?))
    }
}

impl<'lua, T: 'static + UserData + Clone> FromLua<'lua> for T {
    fn from_lua(value: Value<'lua>, _: Context<'lua>) -> Result<T> {
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

impl<'lua> ToLua<'lua> for Error {
    fn to_lua(self, _: Context<'lua>) -> Result<Value<'lua>> {
        Ok(Value::Error(self))
    }
}

impl<'lua> FromLua<'lua> for Error {
    fn from_lua(value: Value<'lua>, lua: Context<'lua>) -> Result<Error> {
        match value {
            Value::Error(err) => Ok(err),
            val => Ok(Error::RuntimeError(
                lua.coerce_string(val)?
                    .and_then(|s| Some(s.to_str().ok()?.to_owned()))
                    .unwrap_or_else(|| "<unprintable error>".to_owned()),
            )),
        }
    }
}

impl<'lua> ToLua<'lua> for bool {
    fn to_lua(self, _: Context<'lua>) -> Result<Value<'lua>> {
        Ok(Value::Boolean(self))
    }
}

impl<'lua> FromLua<'lua> for bool {
    fn from_lua(v: Value, _: Context<'lua>) -> Result<Self> {
        match v {
            Value::Nil => Ok(false),
            Value::Boolean(b) => Ok(b),
            _ => Ok(true),
        }
    }
}

impl<'lua> ToLua<'lua> for LightUserData {
    fn to_lua(self, _: Context<'lua>) -> Result<Value<'lua>> {
        Ok(Value::LightUserData(self))
    }
}

impl<'lua> FromLua<'lua> for LightUserData {
    fn from_lua(value: Value, _: Context<'lua>) -> Result<Self> {
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

impl<'lua> ToLua<'lua> for StdString {
    fn to_lua(self, lua: Context<'lua>) -> Result<Value<'lua>> {
        Ok(Value::String(lua.create_string(&self)?))
    }
}

impl<'lua> FromLua<'lua> for StdString {
    fn from_lua(value: Value<'lua>, lua: Context<'lua>) -> Result<Self> {
        let ty = value.type_name();
        Ok(lua
            .coerce_string(value)?
            .ok_or_else(|| Error::FromLuaConversionError {
                from: ty,
                to: "String",
                message: Some("expected string or number".to_string()),
            })?
            .to_str()?
            .to_owned())
    }
}

impl<'lua, 'a> ToLua<'lua> for &'a str {
    fn to_lua(self, lua: Context<'lua>) -> Result<Value<'lua>> {
        Ok(Value::String(lua.create_string(self)?))
    }
}

impl<'lua> ToLua<'lua> for CString {
    fn to_lua(self, lua: Context<'lua>) -> Result<Value<'lua>> {
        Ok(Value::String(lua.create_string(self.as_bytes())?))
    }
}

impl<'lua> FromLua<'lua> for CString {
    fn from_lua(value: Value<'lua>, lua: Context<'lua>) -> Result<Self> {
        let ty = value.type_name();
        let string = lua
            .coerce_string(value)?
            .ok_or_else(|| Error::FromLuaConversionError {
                from: ty,
                to: "CString",
                message: Some("expected string or number".to_string()),
            })?;

        match CStr::from_bytes_with_nul(string.as_bytes_with_nul()) {
            Ok(s) => Ok(s.into()),
            Err(_) => Err(Error::FromLuaConversionError {
                from: ty,
                to: "CString",
                message: Some("invalid C-style string".to_string()),
            }),
        }
    }
}

impl<'lua, 'a> ToLua<'lua> for &'a CStr {
    fn to_lua(self, lua: Context<'lua>) -> Result<Value<'lua>> {
        Ok(Value::String(lua.create_string(self.to_bytes())?))
    }
}

impl<'lua, 'a> ToLua<'lua> for BString {
    fn to_lua(self, lua: Context<'lua>) -> Result<Value<'lua>> {
        Ok(Value::String(lua.create_string(&self)?))
    }
}

impl<'lua> FromLua<'lua> for BString {
    fn from_lua(value: Value<'lua>, lua: Context<'lua>) -> Result<Self> {
        let ty = value.type_name();
        Ok(BString::from(
            lua.coerce_string(value)?
                .ok_or_else(|| Error::FromLuaConversionError {
                    from: ty,
                    to: "String",
                    message: Some("expected string or number".to_string()),
                })?
                .as_bytes()
                .to_vec(),
        ))
    }
}

impl<'lua, 'a> ToLua<'lua> for &BStr {
    fn to_lua(self, lua: Context<'lua>) -> Result<Value<'lua>> {
        Ok(Value::String(lua.create_string(&self)?))
    }
}

macro_rules! lua_convert_int {
    ($x:ty) => {
        impl<'lua> ToLua<'lua> for $x {
            fn to_lua(self, _: Context<'lua>) -> Result<Value<'lua>> {
                if let Some(i) = cast(self) {
                    #[cfg(rlua_lua51)]
                    let result = Ok(Value::Number(i));
                    #[cfg(any(rlua_lua53, rlua_lua54))]
                    let result = Ok(Value::Integer(i));

                    result
                } else {
                    cast(self)
                        .ok_or_else(|| Error::ToLuaConversionError {
                            from: stringify!($x),
                            to: "number",
                            message: Some("out of range".to_owned()),
                        })
                        .map(Value::Number)
                }
            }
        }

        impl<'lua> FromLua<'lua> for $x {
            fn from_lua(value: Value<'lua>, lua: Context<'lua>) -> Result<Self> {
                let ty = value.type_name();
                (if let Some(i) = lua.coerce_integer(value.clone())? {
                    cast(i)
                } else {
                    cast(lua.coerce_number(value)?.ok_or_else(|| {
                        Error::FromLuaConversionError {
                            from: ty,
                            to: stringify!($x),
                            message: Some(
                                "expected number or string coercible to number".to_string(),
                            ),
                        }
                    })?)
                })
                .ok_or_else(|| Error::FromLuaConversionError {
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
        impl<'lua> ToLua<'lua> for $x {
            fn to_lua(self, _: Context<'lua>) -> Result<Value<'lua>> {
                Ok(Value::Number(self as Number))
            }
        }

        impl<'lua> FromLua<'lua> for $x {
            fn from_lua(value: Value<'lua>, lua: Context<'lua>) -> Result<Self> {
                let ty = value.type_name();
                lua.coerce_number(value)?
                    .ok_or_else(|| Error::FromLuaConversionError {
                        from: ty,
                        to: stringify!($x),
                        message: Some("expected number or string coercible to number".to_string()),
                    })
                    .and_then(|n| {
                        // We want out of range f32 to return an error instead
                        // of inf.
                        if std::mem::size_of::<$x>() < std::mem::size_of::<f64>() {
                            if n.is_finite() && n.abs() > (<$x>::MAX as f64) {
                                return Err(Error::FromLuaConversionError {
                                    from: ty,
                                    to: stringify!($x),
                                    message: Some("number out of range".to_string()),
                                });
                            }
                        }
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

impl<'lua, T: ToLua<'lua>, const N: usize> ToLua<'lua> for [T; N] {
    fn to_lua(self, lua: Context<'lua>) -> Result<Value<'lua>> {
        Ok(Value::Table(lua.create_sequence_from(self)?))
    }
}

impl<'lua, T: FromLua<'lua>, const N: usize> FromLua<'lua> for [T; N] {
    fn from_lua(value: Value<'lua>, _: Context<'lua>) -> Result<Self> {
        if let Value::Table(table) = value {
            // First check the size of the Lua array
            let len = table.len()?;
            if len as usize != N {
                return Err(Error::FromLuaConversionError {
                    from: "Table",
                    to: "Array",
                    message: Some(format!(
                        "Expected array of exactly length {}, got {}",
                        N, len
                    )),
                });
            }

            // Now create a vector out of the Lua array. The use of collect here
            // lets us transform a sequence of Result<T>s into a Result of Vec<T>
            let vec: Vec<T> = table.sequence_values().collect::<Result<Vec<T>>>()?;

            // Finally, turn the Vec<T> into a [T; N]. Unwrap here requires T
            // be Debug, so instead manually match on the result. Note that the
            // Error branch is unreachable since we checked above that the size
            // of the array matches N.
            match vec.try_into() {
                Ok(array) => Ok(array),
                Err(_) => unreachable!(),
            }
        } else {
            Err(Error::FromLuaConversionError {
                from: value.type_name(),
                to: "Array",
                message: Some("expected table".to_string()),
            })
        }
    }
}

impl<'lua, T: ToLua<'lua>> ToLua<'lua> for Vec<T> {
    fn to_lua(self, lua: Context<'lua>) -> Result<Value<'lua>> {
        Ok(Value::Table(lua.create_sequence_from(self)?))
    }
}

impl<'lua, T: FromLuaMulti<'lua>> FromLua<'lua> for Vec<T> {
    fn from_lua(value: Value<'lua>, lua: Context<'lua>) -> Result<Self> {
        if let Value::Table(table) = value {
            let mut values: MultiValue = MultiValue(
                table
                    .sequence_values::<Value>()
                    .filter_map(Result::ok) // Every value in Table is always a Value
                    .collect(),
            );
            let mut result = Vec::new();
            let mut consumed = 0;
            while values.len() > 0 {
                match T::from_lua_multi(values.clone(), lua, &mut consumed) {
                    Ok(it) => {
                        result.push(it);
                        values.drop_front(consumed);
                    }
                    Err(err) => return Err(err),
                }
            }
            Ok(result)
        } else {
            Err(Error::FromLuaConversionError {
                from: value.type_name(),
                to: "Vec",
                message: Some("expected table".to_string()),
            })
        }
    }
}

impl<'lua, K: Eq + Hash + ToLua<'lua>, V: ToLua<'lua>, S: BuildHasher> ToLua<'lua>
    for HashMap<K, V, S>
{
    fn to_lua(self, lua: Context<'lua>) -> Result<Value<'lua>> {
        Ok(Value::Table(lua.create_table_from(self)?))
    }
}

impl<'lua, K: Eq + Hash + FromLua<'lua>, V: FromLua<'lua>, S: BuildHasher + Default> FromLua<'lua>
    for HashMap<K, V, S>
{
    fn from_lua(value: Value<'lua>, _: Context<'lua>) -> Result<Self> {
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

impl<'lua, K: Ord + ToLua<'lua>, V: ToLua<'lua>> ToLua<'lua> for BTreeMap<K, V> {
    fn to_lua(self, lua: Context<'lua>) -> Result<Value<'lua>> {
        Ok(Value::Table(lua.create_table_from(self)?))
    }
}

impl<'lua, K: Ord + FromLua<'lua>, V: FromLua<'lua>> FromLua<'lua> for BTreeMap<K, V> {
    fn from_lua(value: Value<'lua>, _: Context<'lua>) -> Result<Self> {
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

impl<'lua, T: ToLua<'lua>> ToLua<'lua> for Option<T> {
    fn to_lua(self, lua: Context<'lua>) -> Result<Value<'lua>> {
        match self {
            Some(val) => val.to_lua(lua),
            None => Ok(Nil),
        }
    }
}

impl<'lua, T: FromLuaMulti<'lua>> FromLuaMulti<'lua> for Option<T> {
    fn from_lua_multi(
        values: crate::MultiValue<'lua>,
        lua: Context<'lua>,
        consumed: &mut usize,
    ) -> Result<Self> {
        match T::from_lua_multi(values, lua, consumed) {
            Ok(it) => Ok(Some(it)),
            Err(_) => Ok(None),
        }
    }
}
