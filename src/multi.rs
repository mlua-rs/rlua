use error::*;
use lua::*;

#[derive(Debug, Default, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct LNil;

#[derive(Debug, Default, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct LCons<H, T>(pub H, pub T);

impl<'lua> ToLuaMulti<'lua> for () {
    fn to_lua_multi(self, _: &'lua Lua) -> LuaResult<LuaMultiValue> {
        Ok(LuaMultiValue::new())
    }
}

impl<'lua> FromLuaMulti<'lua> for () {
    fn from_lua_multi(_: LuaMultiValue, _: &'lua Lua) -> LuaResult<Self> {
        Ok(())
    }
}

impl<'lua, T: ToLua<'lua>> ToLuaMulti<'lua> for T {
    fn to_lua_multi(self, lua: &'lua Lua) -> LuaResult<LuaMultiValue<'lua>> {
        let mut v = LuaMultiValue::new();
        v.push_back(self.to_lua(lua)?);
        Ok(v)
    }
}

impl<'lua, T: FromLua<'lua>> FromLuaMulti<'lua> for T {
    fn from_lua_multi(mut values: LuaMultiValue<'lua>, lua: &'lua Lua) -> LuaResult<Self> {
        Ok(T::from_lua(values.pop_front().unwrap_or(LuaNil), lua)?)
    }
}

impl<'lua> ToLuaMulti<'lua> for LuaMultiValue<'lua> {
    fn to_lua_multi(self, _: &'lua Lua) -> LuaResult<LuaMultiValue<'lua>> {
        Ok(self)
    }
}

impl<'lua> FromLuaMulti<'lua> for LuaMultiValue<'lua> {
    fn from_lua_multi(values: LuaMultiValue<'lua>, _: &'lua Lua) -> LuaResult<Self> {
        Ok(values)
    }
}

pub struct LuaVariadic<T>(pub Vec<T>);

impl<'lua, T: ToLua<'lua>> ToLuaMulti<'lua> for LuaVariadic<T> {
    fn to_lua_multi(self, lua: &'lua Lua) -> LuaResult<LuaMultiValue<'lua>> {
        self.0.into_iter().map(|e| e.to_lua(lua)).collect()
    }
}

impl<'lua, T: FromLua<'lua>> FromLuaMulti<'lua> for LuaVariadic<T> {
    fn from_lua_multi(values: LuaMultiValue<'lua>, lua: &'lua Lua) -> LuaResult<Self> {
        values
            .into_iter()
            .map(|e| T::from_lua(e, lua))
            .collect::<LuaResult<Vec<T>>>()
            .map(LuaVariadic)
    }
}

impl<'lua> ToLuaMulti<'lua> for LNil {
    fn to_lua_multi(self, _: &'lua Lua) -> LuaResult<LuaMultiValue<'lua>> {
        Ok(LuaMultiValue::new())
    }
}

impl<'lua> FromLuaMulti<'lua> for LNil {
    fn from_lua_multi(_: LuaMultiValue<'lua>, _: &'lua Lua) -> LuaResult<Self> {
        Ok(LNil)
    }
}

impl<'lua, T: ToLuaMulti<'lua>> ToLuaMulti<'lua> for LCons<T, LNil> {
    fn to_lua_multi(self, lua: &'lua Lua) -> LuaResult<LuaMultiValue<'lua>> {
        self.0.to_lua_multi(lua)
    }
}

impl<'lua, T: FromLuaMulti<'lua>> FromLuaMulti<'lua> for LCons<T, LNil> {
    fn from_lua_multi(values: LuaMultiValue<'lua>, lua: &'lua Lua) -> LuaResult<Self> {
        Ok(LCons(T::from_lua_multi(values, lua)?, LNil))
    }
}

impl<'lua, H: ToLua<'lua>, A, B> ToLuaMulti<'lua> for LCons<H, LCons<A, B>>
    where LCons<A, B>: ToLuaMulti<'lua>
{
    fn to_lua_multi(self, lua: &'lua Lua) -> LuaResult<LuaMultiValue<'lua>> {
        let mut results = self.1.to_lua_multi(lua)?;
        results.push_front(self.0.to_lua(lua)?);
        Ok(results)
    }
}

impl<'lua, H: FromLua<'lua>, A, B> FromLuaMulti<'lua> for LCons<H, LCons<A, B>>
    where LCons<A, B>: FromLuaMulti<'lua>
{
    fn from_lua_multi(mut values: LuaMultiValue<'lua>, lua: &'lua Lua) -> LuaResult<Self> {
        let val = H::from_lua(values.pop_front().unwrap_or(LuaNil), lua)?;
        let res = LCons::<A, B>::from_lua_multi(values, lua)?;
        Ok(LCons(val, res))
    }
}

pub fn lua_pack<'lua, H: ToLuaMulti<'lua>>(lua: &'lua Lua, h: H) -> LuaResult<LuaMultiValue<'lua>> {
    h.to_lua_multi(lua)
}

pub fn lua_unpack<'lua, H: FromLuaMulti<'lua>>(lua: &'lua Lua,
                                               a: LuaMultiValue<'lua>)
                                               -> LuaResult<H> {
    H::from_lua_multi(a, lua)
}
