use std::cell::{Ref, RefCell, RefMut};
use std::marker::PhantomData;
use std::collections::HashMap;
use std::string::String as StdString;

use ffi;
use error::*;
use util::*;
use types::{Callback, LuaRef};
use value::{FromLua, FromLuaMulti, ToLua, ToLuaMulti};
use lua::Lua;

/// Kinds of metamethods that can be overridden.
///
/// Currently, this mechanism does not allow overriding the `__gc` metamethod, since there is
/// generally no need to do so: [`UserData`] implementors can instead just implement `Drop`.
///
/// [`UserData`]: trait.UserData.html
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum MetaMethod {
    /// The `+` operator.
    Add,
    /// The `-` operator.
    Sub,
    /// The `*` operator.
    Mul,
    /// The `/` operator.
    Div,
    /// The `%` operator.
    Mod,
    /// The `^` operator.
    Pow,
    /// The unary minus (`-`) operator.
    Unm,
    /// The floor division (//) operator.
    IDiv,
    /// The bitwise AND (&) operator.
    BAnd,
    /// The bitwise OR (|) operator.
    BOr,
    /// The bitwise XOR (binary ~) operator.
    BXor,
    /// The bitwise NOT (unary ~) operator.
    BNot,
    /// The bitwise left shift (<<) operator.
    Shl,
    /// The bitwise right shift (>>) operator.
    Shr,
    /// The string concatenation operator `..`.
    Concat,
    /// The length operator `#`.
    Len,
    /// The `==` operator.
    Eq,
    /// The `<` operator.
    Lt,
    /// The `<=` operator.
    Le,
    /// Index access `obj[key]`.
    Index,
    /// Index write access `obj[key] = value`.
    NewIndex,
    /// The call "operator" `obj(arg1, args2, ...)`.
    Call,
    /// The `__tostring` metamethod.
    ///
    /// This is not an operator, but will be called by methods such as `tostring` and `print`.
    ToString,
}

/// Method registry for [`UserData`] implementors.
///
/// [`UserData`]: trait.UserData.html
pub struct UserDataMethods<'lua, T> {
    pub(crate) methods: HashMap<StdString, Callback<'lua>>,
    pub(crate) meta_methods: HashMap<MetaMethod, Callback<'lua>>,
    pub(crate) _type: PhantomData<T>,
}

impl<'lua, T: UserData> UserDataMethods<'lua, T> {
    /// Add a method which accepts a `&T` as the first parameter.
    ///
    /// Regular methods are implemented by overriding the `__index` metamethod and returning the
    /// accessed method. This allows them to be used with the expected `userdata:method()` syntax.
    ///
    /// If `add_meta_method` is used to override the `__index` metamethod, this approach will fall
    /// back to the user-provided metamethod if no regular method was found.
    pub fn add_method<A, R, M>(&mut self, name: &str, method: M)
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        M: 'static + Send + for<'a> FnMut(&'lua Lua, &'a T, A) -> Result<R>,
    {
        self.methods
            .insert(name.to_owned(), Self::box_method(method));
    }

    /// Add a regular method which accepts a `&mut T` as the first parameter.
    ///
    /// Refer to [`add_method`] for more information about the implementation.
    ///
    /// [`add_method`]: #method.add_method
    pub fn add_method_mut<A, R, M>(&mut self, name: &str, method: M)
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        M: 'static + Send + for<'a> FnMut(&'lua Lua, &'a mut T, A) -> Result<R>,
    {
        self.methods
            .insert(name.to_owned(), Self::box_method_mut(method));
    }

    /// Add a regular method as a function which accepts generic arguments, the first argument will
    /// always be a `UserData` of type T.
    ///
    /// Prefer to use [`add_method`] or [`add_method_mut`] as they are easier to use.
    ///
    /// [`add_method`]: #method.add_method
    /// [`add_method_mut`]: #method.add_method_mut
    pub fn add_function<A, R, F>(&mut self, name: &str, function: F)
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        F: 'static + Send + FnMut(&'lua Lua, A) -> Result<R>,
    {
        self.methods
            .insert(name.to_owned(), Self::box_function(function));
    }

    /// Add a metamethod which accepts a `&T` as the first parameter.
    ///
    /// # Note
    ///
    /// This can cause an error with certain binary metamethods that can trigger if only the right
    /// side has a metatable. To prevent this, use [`add_meta_function`].
    ///
    /// [`add_meta_function`]: #method.add_meta_function
    pub fn add_meta_method<A, R, M>(&mut self, meta: MetaMethod, method: M)
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        M: 'static + Send + for<'a> FnMut(&'lua Lua, &'a T, A) -> Result<R>,
    {
        self.meta_methods.insert(meta, Self::box_method(method));
    }

    /// Add a metamethod as a function which accepts a `&mut T` as the first parameter.
    ///
    /// # Note
    ///
    /// This can cause an error with certain binary metamethods that can trigger if only the right
    /// side has a metatable. To prevent this, use [`add_meta_function`].
    ///
    /// [`add_meta_function`]: #method.add_meta_function
    pub fn add_meta_method_mut<A, R, M>(&mut self, meta: MetaMethod, method: M)
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        M: 'static + Send + for<'a> FnMut(&'lua Lua, &'a mut T, A) -> Result<R>,
    {
        self.meta_methods.insert(meta, Self::box_method_mut(method));
    }

    /// Add a metamethod which accepts generic arguments.
    ///
    /// Metamethods for binary operators can be triggered if either the left or right argument to
    /// the binary operator has a metatable, so the first argument here is not necessarily a
    /// userdata of type `T`.
    pub fn add_meta_function<A, R, F>(&mut self, meta: MetaMethod, function: F)
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        F: 'static + Send + FnMut(&'lua Lua, A) -> Result<R>,
    {
        self.meta_methods.insert(meta, Self::box_function(function));
    }

    fn box_function<A, R, F>(mut function: F) -> Callback<'lua>
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        F: 'static + Send + FnMut(&'lua Lua, A) -> Result<R>,
    {
        Box::new(move |lua, args| function(lua, A::from_lua_multi(args, lua)?)?.to_lua_multi(lua))
    }

    fn box_method<A, R, M>(mut method: M) -> Callback<'lua>
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        M: 'static + Send + for<'a> FnMut(&'lua Lua, &'a T, A) -> Result<R>,
    {
        Box::new(move |lua, mut args| {
            if let Some(front) = args.pop_front() {
                let userdata = AnyUserData::from_lua(front, lua)?;
                let userdata = userdata.borrow::<T>()?;
                method(lua, &userdata, A::from_lua_multi(args, lua)?)?.to_lua_multi(lua)
            } else {
                Err(Error::FromLuaConversionError {
                    from: "missing argument",
                    to: "userdata",
                    message: None,
                })
            }
        })
    }

    fn box_method_mut<A, R, M>(mut method: M) -> Callback<'lua>
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        M: 'static + Send + for<'a> FnMut(&'lua Lua, &'a mut T, A) -> Result<R>,
    {
        Box::new(move |lua, mut args| {
            if let Some(front) = args.pop_front() {
                let userdata = AnyUserData::from_lua(front, lua)?;
                let mut userdata = userdata.borrow_mut::<T>()?;
                method(lua, &mut userdata, A::from_lua_multi(args, lua)?)?.to_lua_multi(lua)
            } else {
                Err(Error::FromLuaConversionError {
                    from: "missing argument",
                    to: "userdata",
                    message: None,
                })
            }
        })
    }
}

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
///     fn add_methods(methods: &mut UserDataMethods<Self>) {
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
/// [`UserDataMethods`]: struct.UserDataMethods.html
pub trait UserData: 'static + Sized {
    /// Adds custom methods and operators specific to this userdata.
    fn add_methods(_methods: &mut UserDataMethods<Self>) {}
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
    pub fn is<T: UserData>(&self) -> Result<bool> {
        match self.inspect(|_: &RefCell<T>| Ok(())) {
            Ok(()) => Ok(true),
            Err(Error::UserDataTypeMismatch) => Ok(false),
            Err(err) => Err(err),
        }
    }

    /// Borrow this userdata immutably if it is of type `T`.
    ///
    /// # Errors
    ///
    /// Returns a `UserDataBorrowError` if the userdata is already mutably borrowed. Returns a
    /// `UserDataTypeMismatch` if the userdata is not of type `T`.
    pub fn borrow<T: UserData>(&self) -> Result<Ref<T>> {
        self.inspect(|cell| Ok(cell.try_borrow().map_err(|_| Error::UserDataBorrowError)?))
    }

    /// Borrow this userdata mutably if it is of type `T`.
    ///
    /// # Errors
    ///
    /// Returns a `UserDataBorrowMutError` if the userdata is already borrowed. Returns a
    /// `UserDataTypeMismatch` if the userdata is not of type `T`.
    pub fn borrow_mut<T: UserData>(&self) -> Result<RefMut<T>> {
        self.inspect(|cell| {
            Ok(cell.try_borrow_mut()
                .map_err(|_| Error::UserDataBorrowMutError)?)
        })
    }

    fn inspect<'a, T, R, F>(&'a self, func: F) -> Result<R>
    where
        T: UserData,
        F: FnOnce(&'a RefCell<T>) -> Result<R>,
    {
        unsafe {
            let lua = self.0.lua;
            stack_err_guard(lua.state, 0, move || {
                check_stack(lua.state, 3);

                lua.push_ref(lua.state, &self.0);

                lua_internal_assert!(
                    lua.state,
                    ffi::lua_getmetatable(lua.state, -1) != 0,
                    "AnyUserData missing metatable"
                );

                ffi::lua_rawgeti(
                    lua.state,
                    ffi::LUA_REGISTRYINDEX,
                    lua.userdata_metatable::<T>()? as ffi::lua_Integer,
                );

                if ffi::lua_rawequal(lua.state, -1, -2) == 0 {
                    ffi::lua_pop(lua.state, 3);
                    Err(Error::UserDataTypeMismatch)
                } else {
                    let res = func(&*get_userdata::<RefCell<T>>(lua.state, -3));
                    ffi::lua_pop(lua.state, 3);
                    res
                }
            })
        }
    }

    /// Sets an associated value to this `AnyUserData`.
    ///
    /// The value may be any Lua value whatsoever, and can be retrieved with [`get_user_value`].
    ///
    /// [`get_user_value`]: #method.get_user_value
    pub fn set_user_value<V: ToLua<'lua>>(&self, v: V) -> Result<()> {
        let lua = self.0.lua;
        unsafe {
            stack_guard(lua.state, 0, || {
                check_stack(lua.state, 2);
                lua.push_ref(lua.state, &self.0);
                lua.push_value(lua.state, v.to_lua(lua)?);
                ffi::lua_setuservalue(lua.state, -2);
                ffi::lua_pop(lua.state, 1);
                Ok(())
            })
        }
    }

    /// Returns an associated value set by [`set_user_value`].
    ///
    /// [`set_user_value`]: #method.set_user_value
    pub fn get_user_value<V: FromLua<'lua>>(&self) -> Result<V> {
        let lua = self.0.lua;
        unsafe {
            stack_guard(lua.state, 0, || {
                check_stack(lua.state, 2);
                lua.push_ref(lua.state, &self.0);
                ffi::lua_getuservalue(lua.state, -1);
                let res = V::from_lua(lua.pop_value(lua.state), lua)?;
                ffi::lua_pop(lua.state, 1);
                Ok(res)
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{MetaMethod, UserData, UserDataMethods};
    use error::ExternalError;
    use string::String;
    use function::Function;
    use lua::Lua;

    #[test]
    fn test_user_data() {
        struct UserData1(i64);
        struct UserData2(Box<i64>);

        impl UserData for UserData1 {};
        impl UserData for UserData2 {};

        let lua = Lua::new();

        let userdata1 = lua.create_userdata(UserData1(1)).unwrap();
        let userdata2 = lua.create_userdata(UserData2(Box::new(2))).unwrap();

        assert!(userdata1.is::<UserData1>().unwrap());
        assert!(!userdata1.is::<UserData2>().unwrap());
        assert!(userdata2.is::<UserData2>().unwrap());
        assert!(!userdata2.is::<UserData1>().unwrap());

        assert_eq!(userdata1.borrow::<UserData1>().unwrap().0, 1);
        assert_eq!(*userdata2.borrow::<UserData2>().unwrap().0, 2);
    }

    #[test]
    fn test_methods() {
        struct MyUserData(i64);

        impl UserData for MyUserData {
            fn add_methods(methods: &mut UserDataMethods<Self>) {
                methods.add_method("get_value", |_, data, ()| Ok(data.0));
                methods.add_method_mut("set_value", |_, data, args| {
                    data.0 = args;
                    Ok(())
                });
            }
        }

        let lua = Lua::new();
        let globals = lua.globals();
        let userdata = lua.create_userdata(MyUserData(42)).unwrap();
        globals.set("userdata", userdata.clone()).unwrap();
        lua.exec::<()>(
            r#"
            function get_it()
                return userdata:get_value()
            end

            function set_it(i)
                return userdata:set_value(i)
            end
        "#,
            None,
        ).unwrap();
        let get = globals.get::<_, Function>("get_it").unwrap();
        let set = globals.get::<_, Function>("set_it").unwrap();
        assert_eq!(get.call::<_, i64>(()).unwrap(), 42);
        userdata.borrow_mut::<MyUserData>().unwrap().0 = 64;
        assert_eq!(get.call::<_, i64>(()).unwrap(), 64);
        set.call::<_, ()>(100).unwrap();
        assert_eq!(get.call::<_, i64>(()).unwrap(), 100);
    }

    #[test]
    fn test_metamethods() {
        #[derive(Copy, Clone)]
        struct MyUserData(i64);

        impl UserData for MyUserData {
            fn add_methods(methods: &mut UserDataMethods<Self>) {
                methods.add_method("get", |_, data, ()| Ok(data.0));
                methods.add_meta_function(
                    MetaMethod::Add,
                    |_, (lhs, rhs): (MyUserData, MyUserData)| Ok(MyUserData(lhs.0 + rhs.0)),
                );
                methods.add_meta_function(
                    MetaMethod::Sub,
                    |_, (lhs, rhs): (MyUserData, MyUserData)| Ok(MyUserData(lhs.0 - rhs.0)),
                );
                methods.add_meta_method(MetaMethod::Index, |_, data, index: String| {
                    if index.to_str()? == "inner" {
                        Ok(data.0)
                    } else {
                        Err(format_err!("no such custom index").to_lua_err())
                    }
                });
            }
        }

        let lua = Lua::new();
        let globals = lua.globals();
        globals.set("userdata1", MyUserData(7)).unwrap();
        globals.set("userdata2", MyUserData(3)).unwrap();
        assert_eq!(
            lua.eval::<MyUserData>("userdata1 + userdata2", None)
                .unwrap()
                .0,
            10
        );
        assert_eq!(
            lua.eval::<MyUserData>("userdata1 - userdata2", None)
                .unwrap()
                .0,
            4
        );
        assert_eq!(lua.eval::<i64>("userdata1:get()", None).unwrap(), 7);
        assert_eq!(lua.eval::<i64>("userdata2.inner", None).unwrap(), 3);
        assert!(lua.eval::<()>("userdata2.nonexist_field", None).is_err());
    }

    #[test]
    fn test_gc_userdata() {
        struct MyUserdata {
            id: u8,
        }

        impl UserData for MyUserdata {
            fn add_methods(methods: &mut UserDataMethods<Self>) {
                methods.add_method("access", |_, this, ()| {
                    assert!(this.id == 123);
                    Ok(())
                });
            }
        }

        let lua = Lua::new();
        {
            let globals = lua.globals();
            globals.set("userdata", MyUserdata { id: 123 }).unwrap();
        }

        assert!(lua.eval::<()>(
            r#"
                local tbl = setmetatable({
                    userdata = userdata
                }, { __gc = function(self)
                    -- resurrect userdata
                    hatch = self.userdata
                end })

                tbl = nil
                userdata = nil  -- make table and userdata collectable
                collectgarbage("collect")
                hatch:access()
            "#,
            None
        ).is_err());
    }

    #[test]
    fn detroys_userdata() {
        struct MyUserdata(Arc<()>);

        impl UserData for MyUserdata {}

        let rc = Arc::new(());

        let lua = Lua::new();
        {
            let globals = lua.globals();
            globals.set("userdata", MyUserdata(rc.clone())).unwrap();
        }

        assert_eq!(Arc::strong_count(&rc), 2);
        drop(lua); // should destroy all objects
        assert_eq!(Arc::strong_count(&rc), 1);
    }

    #[test]
    fn user_value() {
        let lua = Lua::new();

        struct MyUserData;
        impl UserData for MyUserData {}

        let ud = lua.create_userdata(MyUserData).unwrap();
        ud.set_user_value("hello").unwrap();
        assert_eq!(ud.get_user_value::<String>().unwrap(), "hello");
    }
}
