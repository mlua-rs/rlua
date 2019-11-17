use std::os::raw::{c_int, c_void};
use std::sync::{Arc, Mutex};
use std::{fmt, mem, ptr};

use crate::context::Context;
use crate::error::Result;
use crate::ffi;
use crate::value::MultiValue;

/// Type of Lua integer numbers.
pub type Integer = ffi::lua_Integer;
/// Type of Lua floating point numbers.
pub type Number = ffi::lua_Number;

/// A "light" userdata value. Equivalent to an unmanaged raw pointer.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct LightUserData(pub *mut c_void);

pub(crate) type Callback<'lua, 'a> =
    Box<dyn Fn(Context<'lua>, MultiValue<'lua>) -> Result<MultiValue<'lua>> + 'a>;

/// An auto generated key into the Lua registry.
///
/// This is a handle to a value stored inside the Lua registry.  Unlike the `Table` or `Function`
/// handle types, this handle is `Send + Sync + 'static` and can be returned outside of a call to
/// `Lua::context`.  Also, rather than calling methods directly on it, you must instead retrieve the
/// value first by calling [`Context::registry_value`] inside a call to `Lua::context`.
///
/// It is not automatically garbage collected on Drop, but it can be removed with
/// [`Context::remove_registry_value`], and instances not manually removed can be garbage collected
/// with [`Context::expire_registry_values`].
///
/// Be warned, If you place this into Lua via a `UserData` type or a rust callback and rely on
/// [`Context::expire_registry_values`], it is *very easy* to accidentally cause reference cycles
/// that cannot be automatically collected.  The Lua garbage collector is not aware of the registry
/// handle pattern, so holding onto a `RegistryKey` inside Lua may lead to it never being dropped,
/// and it if it is not droped, [`Context::expire_registry_values`] will never remove the value from
/// the registry, leading to an uncollectable cycle.  Instead of placing a `RegistryKey` into Lua
/// and relying on it being automatically dropped, prefer APIs which the Lua garbage collector
/// understands, such as [`UserData::set_user_value`] / [`UserData::get_user_value`] for UserData
/// types and [`Function::bind`] for callbacks.
///
/// [`Context::registry_value`]: struct.Context.html#method.registry_value
/// [`Context::remove_registry_value`]: struct.Context.html#method.remove_registry_value
/// [`Context::expire_registry_values`]: struct.Context.html#method.expire_registry_values
/// [`Function::bind`]: struct.Function.html#method.bind
/// [`UserData::set_user_value`]: struct.UserData.html#method.set_user_value
/// [`UserData::get_user_value`]: struct.UserData.html#method.get_user_value
pub struct RegistryKey {
    pub(crate) registry_id: c_int,
    pub(crate) unref_list: Arc<Mutex<Option<Vec<c_int>>>>,
}

impl fmt::Debug for RegistryKey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "RegistryKey({})", self.registry_id)
    }
}

impl Drop for RegistryKey {
    fn drop(&mut self) {
        if let Some(list) = rlua_expect!(self.unref_list.lock(), "unref_list poisoned").as_mut() {
            list.push(self.registry_id);
        }
    }
}

impl RegistryKey {
    // Destroys the RegistryKey without adding to the drop list
    pub(crate) fn take(self) -> c_int {
        let registry_id = self.registry_id;
        unsafe {
            ptr::read(&self.unref_list);
            mem::forget(self);
        }
        registry_id
    }
}

pub(crate) struct LuaRef<'lua> {
    pub(crate) lua: Context<'lua>,
    pub(crate) index: c_int,
}

impl<'lua> fmt::Debug for LuaRef<'lua> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Ref({})", self.index)
    }
}

impl<'lua> Clone for LuaRef<'lua> {
    fn clone(&self) -> Self {
        self.lua.clone_ref(self)
    }
}

impl<'lua> Drop for LuaRef<'lua> {
    fn drop(&mut self) {
        self.lua.drop_ref(self)
    }
}
