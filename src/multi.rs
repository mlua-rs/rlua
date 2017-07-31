use std::result::Result as StdResult;

use error::*;
use lua::*;

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

macro_rules! impl_tuple {
    () => (
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
    );

    ($last:ident $($name:ident)*) => (
        impl<'lua, $($name,)* $last> ToLuaMulti<'lua> for ($($name,)* $last,)
            where $($name: ToLua<'lua>,)*
                  $last: ToLuaMulti<'lua>
        {
            #[allow(unused_mut)]
            #[allow(non_snake_case)]
            fn to_lua_multi(self, lua: &'lua Lua) -> Result<MultiValue<'lua>> {
                let ($($name,)* $last,) = self;

                let mut results = $last.to_lua_multi(lua)?;
                push_reverse!(results, $($name.to_lua(lua)?,)*);
                Ok(results)
            }
        }

        impl<'lua, $($name,)* $last> FromLuaMulti<'lua> for ($($name,)* $last,)
            where $($name: FromLua<'lua>,)*
                  $last: FromLuaMulti<'lua>
        {
            #[allow(unused_mut)]
            #[allow(non_snake_case)]
            fn from_lua_multi(mut values: MultiValue<'lua>, lua: &'lua Lua) -> Result<Self> {
                $(let $name = values.pop_front().unwrap_or(Nil);)*
                let $last = FromLuaMulti::from_lua_multi(values, lua)?;
                Ok(($(FromLua::from_lua($name, lua)?,)* $last,))
            }
        }
    );
}

macro_rules! push_reverse {
    ($multi_value:expr, $first:expr, $($rest:expr,)*) => (
        push_reverse!($multi_value, $($rest,)*);
        $multi_value.push_front($first);
    );

    ($multi_value:expr, $first:expr) => (
        $multi_value.push_front($first);
    );

    ($multi_value:expr,) => ();
}

impl_tuple! { }
impl_tuple! { A }
impl_tuple! { A B }
impl_tuple! { A B C }
impl_tuple! { A B C D }
impl_tuple! { A B C D E }
impl_tuple! { A B C D E F }
impl_tuple! { A B C D E F G }
impl_tuple! { A B C D E F G H }
impl_tuple! { A B C D E F G H I }
impl_tuple! { A B C D E F G H I J }
impl_tuple! { A B C D E F G H I J K }
impl_tuple! { A B C D E F G H I J K L }
impl_tuple! { A B C D E F G H I J K L M }
impl_tuple! { A B C D E F G H I J K L M N }
impl_tuple! { A B C D E F G H I J K L M N O }
impl_tuple! { A B C D E F G H I J K L M N O P }
