use std::cell::{Ref, RefCell, RefMut};

use error::{Error, Result};
use ffi;
use methods::UserDataMethods;
use types::LuaRef;
use util::{assert_stack, get_userdata, StackGuard};
use value::{FromLua, ToLua};

/// Trait for custom userdata types.
///
/// By implementing this trait, a struct becomes eligible for use inside Lua code. Implementations
/// of [`ToLua`] and [`FromLua`] are automatically provided.
///
/// # Examples
///
/// ```
/// # extern crate rlua;
/// # use rlua::{Lua, UserData, Result};
/// # fn try_main() -> Result<()> {
/// struct MyUserData(i32);
///
/// impl UserData for MyUserData {}
///
/// let lua = Lua::new();
///
/// // `MyUserData` now implements `ToLua`:
/// lua.globals().set("myobject", MyUserData(123))?;
///
/// lua.exec::<()>("assert(type(myobject) == 'userdata')", None)?;
/// # Ok(())
/// # }
/// # fn main() {
/// #     try_main().unwrap();
/// # }
/// ```
///
/// Custom methods and operators can be provided by implementing `add_methods` (refer to
/// [`UserDataMethods`] for more information):
///
/// ```
/// # extern crate rlua;
/// # use rlua::{Lua, MetaMethod, UserData, UserDataMethods, Result};
/// # fn try_main() -> Result<()> {
/// struct MyUserData(i32);
///
/// impl UserData for MyUserData {
///     fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
///         methods.add_method("get", |_, this, _: ()| {
///             Ok(this.0)
///         });
///
///         methods.add_method_mut("add", |_, this, value: i32| {
///             this.0 += value;
///             Ok(())
///         });
///
///         methods.add_meta_method(MetaMethod::Add, |_, this, value: i32| {
///             Ok(this.0 + value)
///         });
///     }
/// }
///
/// let lua = Lua::new();
///
/// lua.globals().set("myobject", MyUserData(123))?;
///
/// lua.exec::<()>(r#"
///     assert(myobject:get() == 123)
///     myobject:add(7)
///     assert(myobject:get() == 130)
///     assert(myobject + 10 == 140)
/// "#, None)?;
/// # Ok(())
/// # }
/// # fn main() {
/// #     try_main().unwrap();
/// # }
/// ```
///
/// [`ToLua`]: trait.ToLua.html
/// [`FromLua`]: trait.FromLua.html
/// [`UserDataMethods`]: trait.UserDataMethods.html
pub trait UserData: Sized {
    /// Adds custom methods and operators specific to this userdata.
    fn add_methods<'lua, T: UserDataMethods<'lua, Self>>(_methods: &mut T) {}
}

/// Handle to an internal Lua userdata for any type that implements [`UserData`].
///
/// Similar to `std::any::Any`, this provides an interface for dynamic type checking via the [`is`]
/// and [`borrow`] methods.
///
/// Internally, instances are stored in a `RefCell`, to best match the mutable semantics of the Lua
/// language.
///
/// # Note
///
/// This API should only be used when necessary. Implementing [`UserData`] already allows defining
/// methods which check the type and acquire a borrow behind the scenes.
///
/// [`UserData`]: trait.UserData.html
/// [`is`]: #method.is
/// [`borrow`]: #method.borrow
#[derive(Clone, Debug)]
pub struct AnyUserData<'lua>(pub(crate) LuaRef<'lua>);

impl<'lua> AnyUserData<'lua> {
    /// Checks whether the type of this userdata is `T`.
    pub fn is<T: 'static + UserData>(&self) -> bool {
        match self.inspect(|_: &RefCell<T>| Ok(())) {
            Ok(()) => true,
            Err(Error::UserDataTypeMismatch) => false,
            Err(_) => unreachable!(),
        }
    }

    /// Borrow this userdata immutably if it is of type `T`.
    ///
    /// # Errors
    ///
    /// Returns a `UserDataBorrowError` if the userdata is already mutably borrowed. Returns a
    /// `UserDataTypeMismatch` if the userdata is not of type `T`.
    pub fn borrow<T: 'static + UserData>(&self) -> Result<Ref<T>> {
        self.inspect(|cell| Ok(cell.try_borrow().map_err(|_| Error::UserDataBorrowError)?))
    }

    /// Borrow this userdata mutably if it is of type `T`.
    ///
    /// # Errors
    ///
    /// Returns a `UserDataBorrowMutError` if the userdata is already borrowed. Returns a
    /// `UserDataTypeMismatch` if the userdata is not of type `T`.
    pub fn borrow_mut<T: 'static + UserData>(&self) -> Result<RefMut<T>> {
        self.inspect(|cell| {
            Ok(cell
                .try_borrow_mut()
                .map_err(|_| Error::UserDataBorrowMutError)?)
        })
    }

    /// Sets an associated value to this `AnyUserData`.
    ///
    /// The value may be any Lua value whatsoever, and can be retrieved with [`get_user_value`].
    ///
    /// [`get_user_value`]: #method.get_user_value
    pub fn set_user_value<V: ToLua<'lua>>(&self, v: V) -> Result<()> {
        let lua = self.0.lua;
        let v = v.to_lua(lua)?;
        unsafe {
            let _sg = StackGuard::new(lua.state);
            assert_stack(lua.state, 2);
            lua.push_ref(&self.0);
            lua.push_value(v);
            ffi::lua_setuservalue(lua.state, -2);
            Ok(())
        }
    }

    /// Returns an associated value set by [`set_user_value`].
    ///
    /// [`set_user_value`]: #method.set_user_value
    pub fn get_user_value<V: FromLua<'lua>>(&self) -> Result<V> {
        let lua = self.0.lua;
        let res = unsafe {
            let _sg = StackGuard::new(lua.state);
            assert_stack(lua.state, 3);
            lua.push_ref(&self.0);
            ffi::lua_getuservalue(lua.state, -1);
            lua.pop_value()
        };
        V::from_lua(res, lua)
    }

    fn inspect<'a, T, R, F>(&'a self, func: F) -> Result<R>
    where
        T: 'static + UserData,
        F: FnOnce(&'a RefCell<T>) -> Result<R>,
    {
        unsafe {
            let lua = self.0.lua;
            let _sg = StackGuard::new(lua.state);
            assert_stack(lua.state, 3);

            lua.push_ref(&self.0);

            if ffi::lua_getmetatable(lua.state, -1) == 0 {
                Err(Error::UserDataTypeMismatch)
            } else {
                ffi::lua_rawgeti(
                    lua.state,
                    ffi::LUA_REGISTRYINDEX,
                    lua.userdata_metatable::<T>()? as ffi::lua_Integer,
                );

                if ffi::lua_rawequal(lua.state, -1, -2) == 0 {
                    Err(Error::UserDataTypeMismatch)
                } else {
                    func(&*get_userdata::<RefCell<T>>(lua.state, -3))
                }
            }
        }
    }
}
