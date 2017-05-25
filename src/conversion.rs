use std::collections::{HashMap, BTreeMap, HashSet, BTreeSet};
use std::iter::FromIterator;
use std::hash::Hash;

use error::*;
use lua::*;

impl<'lua> ToLua<'lua> for LuaValue<'lua> {
    fn to_lua(self, _: &'lua Lua) -> LuaResult<LuaValue<'lua>> {
        Ok(self)
    }
}

impl<'lua> FromLua<'lua> for LuaValue<'lua> {
    fn from_lua(lua_value: LuaValue<'lua>, _: &'lua Lua) -> LuaResult<Self> {
        Ok(lua_value)
    }
}

impl<'lua> ToLua<'lua> for LuaString<'lua> {
    fn to_lua(self, _: &'lua Lua) -> LuaResult<LuaValue<'lua>> {
        Ok(LuaValue::String(self))
    }
}

impl<'lua> FromLua<'lua> for LuaString<'lua> {
    fn from_lua(value: LuaValue<'lua>, lua: &'lua Lua) -> LuaResult<LuaString<'lua>> {
        lua.coerce_string(value)
    }
}

impl<'lua> ToLua<'lua> for LuaTable<'lua> {
    fn to_lua(self, _: &'lua Lua) -> LuaResult<LuaValue> {
        Ok(LuaValue::Table(self))
    }
}

impl<'lua> FromLua<'lua> for LuaTable<'lua> {
    fn from_lua(value: LuaValue<'lua>, _: &'lua Lua) -> LuaResult<LuaTable<'lua>> {
        match value {
            LuaValue::Table(table) => Ok(table),
            _ => Err("cannot convert lua value to table".into()),
        }
    }
}

impl<'lua> ToLua<'lua> for LuaFunction<'lua> {
    fn to_lua(self, _: &'lua Lua) -> LuaResult<LuaValue<'lua>> {
        Ok(LuaValue::Function(self))
    }
}

impl<'lua> FromLua<'lua> for LuaFunction<'lua> {
    fn from_lua(value: LuaValue<'lua>, _: &'lua Lua) -> LuaResult<LuaFunction<'lua>> {
        match value {
            LuaValue::Function(table) => Ok(table),
            _ => Err("cannot convert lua value to function".into()),
        }
    }
}

impl<'lua> ToLua<'lua> for LuaUserData<'lua> {
    fn to_lua(self, _: &'lua Lua) -> LuaResult<LuaValue<'lua>> {
        Ok(LuaValue::UserData(self))
    }
}

impl<'lua> FromLua<'lua> for LuaUserData<'lua> {
    fn from_lua(value: LuaValue<'lua>, _: &'lua Lua) -> LuaResult<LuaUserData<'lua>> {
        match value {
            LuaValue::UserData(ud) => Ok(ud),
            _ => Err("cannot convert lua value to userdata".into()),
        }
    }
}

impl<'lua> ToLua<'lua> for LuaThread<'lua> {
    fn to_lua(self, _: &'lua Lua) -> LuaResult<LuaValue<'lua>> {
        Ok(LuaValue::Thread(self))
    }
}

impl<'lua> FromLua<'lua> for LuaThread<'lua> {
    fn from_lua(value: LuaValue<'lua>, _: &'lua Lua) -> LuaResult<LuaThread<'lua>> {
        match value {
            LuaValue::Thread(t) => Ok(t),
            _ => Err("cannot convert lua value to thread".into()),
        }
    }
}

impl<'lua, T: LuaUserDataType> ToLua<'lua> for T {
    fn to_lua(self, lua: &'lua Lua) -> LuaResult<LuaValue<'lua>> {
        lua.create_userdata(self).map(LuaValue::UserData)
    }
}

impl<'lua, T: LuaUserDataType + Copy> FromLua<'lua> for T {
    fn from_lua(value: LuaValue<'lua>, _: &'lua Lua) -> LuaResult<T> {
        match value {
            LuaValue::UserData(ud) => Ok(*ud.borrow::<T>()?),
            _ => Err("cannot convert lua value to userdata".into()),
        }
    }
}

impl<'lua> ToLua<'lua> for bool {
    fn to_lua(self, _: &'lua Lua) -> LuaResult<LuaValue<'lua>> {
        Ok(LuaValue::Boolean(self))
    }
}

impl<'lua> FromLua<'lua> for bool {
    fn from_lua(v: LuaValue, _: &'lua Lua) -> LuaResult<Self> {
        match v {
            LuaValue::Nil => Ok(false),
            LuaValue::Boolean(b) => Ok(b),
            _ => Ok(true),
        }
    }
}

impl<'lua> ToLua<'lua> for String {
    fn to_lua(self, lua: &'lua Lua) -> LuaResult<LuaValue<'lua>> {
        Ok(LuaValue::String(lua.create_string(&self)?))
    }
}

impl<'lua> FromLua<'lua> for String {
    fn from_lua(value: LuaValue<'lua>, lua: &'lua Lua) -> LuaResult<Self> {
        Ok(lua.coerce_string(value)?.get()?.to_owned())
    }
}

impl<'lua, 'a> ToLua<'lua> for &'a str {
    fn to_lua(self, lua: &'lua Lua) -> LuaResult<LuaValue<'lua>> {
        Ok(LuaValue::String(lua.create_string(self)?))
    }
}

macro_rules! lua_convert_int {
    ($x: ty) => {
        impl<'lua> ToLua<'lua> for $x {
            fn to_lua(self, _: &'lua Lua) -> LuaResult<LuaValue<'lua>> {
                Ok(LuaValue::Integer(self as LuaInteger))
            }
        }

        impl<'lua> FromLua<'lua> for $x {
            fn from_lua(value: LuaValue<'lua>, lua: &'lua Lua) -> LuaResult<Self> {
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
            fn to_lua(self, _: &'lua Lua) -> LuaResult<LuaValue<'lua>> {
                Ok(LuaValue::Number(self as LuaNumber))
            }
        }

        impl<'lua> FromLua<'lua> for $x {
            fn from_lua(value: LuaValue<'lua>, lua: &'lua Lua) -> LuaResult<Self> {
                Ok(lua.coerce_number(value)? as $x)
            }
        }
    }
}

lua_convert_float!(f32);
lua_convert_float!(f64);

impl<'lua, T: ToLua<'lua>> ToLua<'lua> for Vec<T> {
    fn to_lua(self, lua: &'lua Lua) -> LuaResult<LuaValue<'lua>> {
        Ok(LuaValue::Table(lua.create_array_table(self)?))
    }
}

impl<'lua, T: FromLua<'lua>> FromLua<'lua> for Vec<T> {
    fn from_lua(value: LuaValue<'lua>, _: &'lua Lua) -> LuaResult<Self> {
        if let LuaValue::Table(table) = value {
            table.array_values()
        } else {
            Err("cannot convert lua value to table for Vec".into())
        }
    }
}

impl<'lua, T: Eq + Hash + ToLua<'lua>> ToLua<'lua> for HashSet<T> {
    fn to_lua(self, lua: &'lua Lua) -> LuaResult<LuaValue<'lua>> {
        Ok(LuaValue::Table(lua.create_array_table(self)?))
    }
}

impl<'lua, T: Eq + Hash + FromLua<'lua>> FromLua<'lua> for HashSet<T> {
    fn from_lua(value: LuaValue<'lua>, _: &'lua Lua) -> LuaResult<Self> {
        if let LuaValue::Table(table) = value {
            Ok(HashSet::from_iter(table.array_values()?))
        } else {
            Err("cannot convert lua value to table for HashSet".into())
        }
    }
}

impl<'lua, T: Ord + ToLua<'lua>> ToLua<'lua> for BTreeSet<T> {
    fn to_lua(self, lua: &'lua Lua) -> LuaResult<LuaValue<'lua>> {
        Ok(LuaValue::Table(lua.create_array_table(self)?))
    }
}

impl<'lua, T: Ord + FromLua<'lua>> FromLua<'lua> for BTreeSet<T> {
    fn from_lua(value: LuaValue<'lua>, _: &'lua Lua) -> LuaResult<Self> {
        if let LuaValue::Table(table) = value {
            Ok(BTreeSet::from_iter(table.array_values()?))
        } else {
            Err("cannot convert lua value to table for BTreeSet".into())
        }
    }
}

impl<'lua, K: Eq + Hash + ToLua<'lua>, V: ToLua<'lua>> ToLua<'lua> for HashMap<K, V> {
    fn to_lua(self, lua: &'lua Lua) -> LuaResult<LuaValue<'lua>> {
        Ok(LuaValue::Table(lua.create_table(self)?))
    }
}

impl<'lua, K: Eq + Hash + FromLua<'lua>, V: FromLua<'lua>> FromLua<'lua> for HashMap<K, V> {
    fn from_lua(value: LuaValue<'lua>, _: &'lua Lua) -> LuaResult<Self> {
        if let LuaValue::Table(table) = value {
            Ok(HashMap::from_iter(table.pairs()?))
        } else {
            Err("cannot convert lua value to table for HashMap".into())
        }
    }
}

impl<'lua, K: Ord + ToLua<'lua>, V: ToLua<'lua>> ToLua<'lua> for BTreeMap<K, V> {
    fn to_lua(self, lua: &'lua Lua) -> LuaResult<LuaValue<'lua>> {
        Ok(LuaValue::Table(lua.create_table(self)?))
    }
}

impl<'lua, K: Ord + FromLua<'lua>, V: FromLua<'lua>> FromLua<'lua> for BTreeMap<K, V> {
    fn from_lua(value: LuaValue<'lua>, _: &'lua Lua) -> LuaResult<Self> {
        if let LuaValue::Table(table) = value {
            Ok(BTreeMap::from_iter(table.pairs()?))
        } else {
            Err("cannot convert lua value to table for BTreeMap".into())
        }
    }
}

impl<'lua, T: ToLua<'lua>> ToLua<'lua> for Option<T> {
    fn to_lua(self, lua: &'lua Lua) -> LuaResult<LuaValue<'lua>> {
        match self {
            Some(val) => val.to_lua(lua),
            None => Ok(LuaNil),
        }
    }
}

impl<'lua, T: FromLua<'lua>> FromLua<'lua> for Option<T> {
    fn from_lua(value: LuaValue<'lua>, lua: &'lua Lua) -> LuaResult<Self> {
        match value {
            LuaNil => Ok(None),
            value => Ok(Some(T::from_lua(value, lua)?)),
        }
    }
}
