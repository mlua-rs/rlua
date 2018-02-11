use std::fmt;
use std::os::raw::{c_int, c_void};
use std::sync::{Arc, Mutex};

use ffi;
use error::Result;
use value::MultiValue;
use lua::Lua;

/// Type of Lua integer numbers.
pub type Integer = ffi::lua_Integer;
/// Type of Lua floating point numbers.
pub type Number = ffi::lua_Number;

/// A "light" userdata value. Equivalent to an unmanaged raw pointer.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct LightUserData(pub *mut c_void);

/// An auto generated key into the Lua registry.
///
/// This is a handle into a value stored inside the Lua registry, similar to the normal handle types
/// like `Table` or `Function`.  The difference is that this handle does not require holding a
/// reference to a parent `Lua` instance, and thus is managed differently.  Though it is more
/// difficult to use than the normal handle types, it is Send + Sync + 'static, which means that it
/// can be used in many situations where it would be impossible to store a regular handle value.
///
/// Be warned, If you place this into Lua via a `UserData` type, it is *very easy* to accidentally
/// cause reference cycles that the Lua garbage collector cannot resolve.  Instead of placing a
/// `RegistryKey` into a `UserData` type, prefer instead to use `UserData::set_user_value` /
/// `UserData::get_user_value`.
pub struct RegistryKey {
    pub(crate) registry_id: c_int,
    pub(crate) unref_list: Arc<Mutex<Option<Vec<c_int>>>>,
    pub(crate) drop_unref: bool,
}

impl Drop for RegistryKey {
    fn drop(&mut self) {
        if self.drop_unref {
            if let Some(list) = self.unref_list.lock().unwrap().as_mut() {
                list.push(self.registry_id);
            }
        }
    }
}

pub(crate) type Callback<'lua, 'a> =
    Box<Fn(&'lua Lua, MultiValue<'lua>) -> Result<MultiValue<'lua>> + 'a>;

pub(crate) struct LuaRef<'lua> {
    pub lua: &'lua Lua,
    pub registry_id: c_int,
    pub drop_unref: bool,
}

impl<'lua> fmt::Debug for LuaRef<'lua> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "LuaRef({})", self.registry_id)
    }
}

impl<'lua> Clone for LuaRef<'lua> {
    fn clone(&self) -> Self {
        if self.drop_unref {
            unsafe {
                self.lua.push_ref(self.lua.state, self);
                self.lua.pop_ref(self.lua.state)
            }
        } else {
            LuaRef {
                lua: self.lua,
                registry_id: self.registry_id,
                drop_unref: self.drop_unref,
            }
        }
    }
}

impl<'lua> Drop for LuaRef<'lua> {
    fn drop(&mut self) {
        if self.drop_unref {
            unsafe {
                ffi::luaL_unref(self.lua.state, ffi::LUA_REGISTRYINDEX, self.registry_id);
            }
        }
    }
}
