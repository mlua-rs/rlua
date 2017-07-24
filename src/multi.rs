use std::result::{Result as StdResult};

use hlist_macro::{HNil, HCons};

use error::*;
use lua::*;

impl<'lua> ToLuaMulti<'lua> for () {
    fn to_lua_multi(self, _: &'lua Lua) -> Result<MultiValue> {
        Ok(MultiValue::new())
    }
}

impl<'lua> FromLuaMulti<'lua> for () {
    fn from_lua_multi(_: MultiValue, _: &'lua Lua) -> Result<Self> {
        Ok(())
    }
}

/// Result is convertible to `MultiValue` following the common lua idiom of returning the result
/// on success, or in the case of an error, returning nil followed by the error
impl<'lua, T: ToLua<'lua>, E: ToLua<'lua>> ToLuaMulti<'lua> for StdResult<T, E> {
    fn to_lua_multi(self, lua: &'lua Lua) -> Result<MultiValue<'lua>> {
        let mut result = MultiValue::new();

        match self {
            Ok(v) => result.push_back(v.to_lua(lua)?),
            Err(e) => {
                result.push_back(Nil);
                result.push_back(e.to_lua(lua)?);
            }
        }

        Ok(result)
    }
}

impl<'lua, T: ToLua<'lua>> ToLuaMulti<'lua> for T {
    fn to_lua_multi(self, lua: &'lua Lua) -> Result<MultiValue<'lua>> {
        let mut v = MultiValue::new();
        v.push_back(self.to_lua(lua)?);
        Ok(v)
    }
}

impl<'lua, T: FromLua<'lua>> FromLuaMulti<'lua> for T {
    fn from_lua_multi(mut values: MultiValue<'lua>, lua: &'lua Lua) -> Result<Self> {
        Ok(T::from_lua(values.pop_front().unwrap_or(Nil), lua)?)
    }
}

impl<'lua> ToLuaMulti<'lua> for MultiValue<'lua> {
    fn to_lua_multi(self, _: &'lua Lua) -> Result<MultiValue<'lua>> {
        Ok(self)
    }
}

impl<'lua> FromLuaMulti<'lua> for MultiValue<'lua> {
    fn from_lua_multi(values: MultiValue<'lua>, _: &'lua Lua) -> Result<Self> {
        Ok(values)
    }
}

/// Can be used to pass variadic values to or receive variadic values from Lua, where the type of
/// the values is all the same and the number of values is defined at runtime.  This can be included
/// in an hlist when unpacking, but must be the final entry, and will consume the rest of the
/// parameters given.
pub struct Variadic<T>(pub Vec<T>);

impl<'lua, T: ToLua<'lua>> ToLuaMulti<'lua> for Variadic<T> {
    fn to_lua_multi(self, lua: &'lua Lua) -> Result<MultiValue<'lua>> {
        self.0.into_iter().map(|e| e.to_lua(lua)).collect()
    }
}

impl<'lua, T: FromLua<'lua>> FromLuaMulti<'lua> for Variadic<T> {
    fn from_lua_multi(values: MultiValue<'lua>, lua: &'lua Lua) -> Result<Self> {
        values
            .into_iter()
            .map(|e| T::from_lua(e, lua))
            .collect::<Result<Vec<T>>>()
            .map(Variadic)
    }
}

impl<'lua> ToLuaMulti<'lua> for HNil {
    fn to_lua_multi(self, _: &'lua Lua) -> Result<MultiValue<'lua>> {
        Ok(MultiValue::new())
    }
}

impl<'lua> FromLuaMulti<'lua> for HNil {
    fn from_lua_multi(_: MultiValue<'lua>, _: &'lua Lua) -> Result<Self> {
        Ok(HNil)
    }
}

impl<'lua, T: ToLuaMulti<'lua>> ToLuaMulti<'lua> for HCons<T, HNil> {
    fn to_lua_multi(self, lua: &'lua Lua) -> Result<MultiValue<'lua>> {
        self.0.to_lua_multi(lua)
    }
}

impl<'lua, T: FromLuaMulti<'lua>> FromLuaMulti<'lua> for HCons<T, HNil> {
    fn from_lua_multi(values: MultiValue<'lua>, lua: &'lua Lua) -> Result<Self> {
        Ok(HCons(T::from_lua_multi(values, lua)?, HNil))
    }
}

impl<'lua, H: ToLua<'lua>, A, B> ToLuaMulti<'lua> for HCons<H, HCons<A, B>>
    where HCons<A, B>: ToLuaMulti<'lua>
{
    fn to_lua_multi(self, lua: &'lua Lua) -> Result<MultiValue<'lua>> {
        let mut results = self.1.to_lua_multi(lua)?;
        results.push_front(self.0.to_lua(lua)?);
        Ok(results)
    }
}

impl<'lua, H: FromLua<'lua>, A, B> FromLuaMulti<'lua> for HCons<H, HCons<A, B>>
    where HCons<A, B>: FromLuaMulti<'lua>
{
    fn from_lua_multi(mut values: MultiValue<'lua>, lua: &'lua Lua) -> Result<Self> {
        let val = H::from_lua(values.pop_front().unwrap_or(Nil), lua)?;
        let res = HCons::<A, B>::from_lua_multi(values, lua)?;
        Ok(HCons(val, res))
    }
}
