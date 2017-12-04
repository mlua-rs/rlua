use std::fmt;
use std::os::raw::{c_int, c_void};

use ffi;
use error::Result;
use lua::{Lua, MultiValue};

/// Type of Lua integer numbers.
pub type Integer = ffi::lua_Integer;
/// Type of Lua floating point numbers.
pub type Number = ffi::lua_Number;

/// A "light" userdata value. Equivalent to an unmanaged raw pointer.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct LightUserData(pub *mut c_void);

pub(crate) type Callback<'lua> = Box<
    FnMut(&'lua Lua, MultiValue<'lua>) -> Result<MultiValue<'lua>> + 'lua,
>;

pub(crate) struct LuaRef<'lua> {
    pub lua: &'lua Lua,
    pub registry_id: c_int,
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

#[cfg(test)]
mod tests {
    use super::LightUserData;
    use function::Function;
    use lua::Lua;

    use std::os::raw::c_void;

    #[test]
    fn test_lightuserdata() {
        let lua = Lua::new();
        let globals = lua.globals();
        lua.exec::<()>(
            r#"
            function id(a)
                return a
            end
        "#,
            None,
        ).unwrap();
        let res = globals
            .get::<_, Function>("id")
            .unwrap()
            .call::<_, LightUserData>(LightUserData(42 as *mut c_void))
            .unwrap();
        assert_eq!(res, LightUserData(42 as *mut c_void));
    }
}
