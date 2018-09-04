use std::cell::RefCell;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::string::String as StdString;

use error::{Error, Result};
use lua::Lua;
use types::Callback;
use userdata::{AnyUserData, UserData};
use value::{FromLua, FromLuaMulti, MultiValue, ToLuaMulti};

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

pub(crate) fn meta_method_name(meta: MetaMethod) -> &'static str {
    match meta {
        MetaMethod::Add => "__add",
        MetaMethod::Sub => "__sub",
        MetaMethod::Mul => "__mul",
        MetaMethod::Div => "__div",
        MetaMethod::Mod => "__mod",
        MetaMethod::Pow => "__pow",
        MetaMethod::Unm => "__unm",
        MetaMethod::IDiv => "__idiv",
        MetaMethod::BAnd => "__band",
        MetaMethod::BOr => "__bor",
        MetaMethod::BXor => "__bxor",
        MetaMethod::BNot => "__bnot",
        MetaMethod::Shl => "__shl",
        MetaMethod::Shr => "__shr",
        MetaMethod::Concat => "__concat",
        MetaMethod::Len => "__len",
        MetaMethod::Eq => "__eq",
        MetaMethod::Lt => "__lt",
        MetaMethod::Le => "__le",
        MetaMethod::Index => "__index",
        MetaMethod::NewIndex => "__newindex",
        MetaMethod::Call => "__call",
        MetaMethod::ToString => "__tostring",
    }
}

/// Method registry for [`UserData`] implementors.
///
/// [`UserData`]: trait.UserData.html
pub trait UserDataMethods<'lua, T: UserData> {
    /// Add a method which accepts a `&T` as the first parameter.
    ///
    /// Regular methods are implemented by overriding the `__index` metamethod and returning the
    /// accessed method. This allows them to be used with the expected `userdata:method()` syntax.
    ///
    /// If `add_meta_method` is used to set the `__index` metamethod, the `__index` metamethod will
    /// be used as a fall-back if no regular method is found.
    fn add_method<A, R, M>(&mut self, name: &str, method: M)
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        M: 'static + Send + Fn(&'lua Lua, &T, A) -> Result<R>;

    /// Add a regular method which accepts a `&mut T` as the first parameter.
    ///
    /// Refer to [`add_method`] for more information about the implementation.
    ///
    /// [`add_method`]: #method.add_method
    fn add_method_mut<A, R, M>(&mut self, name: &str, method: M)
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        M: 'static + Send + FnMut(&'lua Lua, &mut T, A) -> Result<R>;

    /// Add a regular method as a function which accepts generic arguments, the first argument will
    /// always be a `UserData` of type T.
    ///
    /// Prefer to use [`add_method`] or [`add_method_mut`] as they are easier to use.
    ///
    /// [`add_method`]: #method.add_method
    /// [`add_method_mut`]: #method.add_method_mut
    fn add_function<A, R, F>(&mut self, name: &str, function: F)
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        F: 'static + Send + Fn(&'lua Lua, A) -> Result<R>;

    /// Add a regular method as a mutable function which accepts generic arguments, the first
    /// argument will always be a `UserData` of type T.
    ///
    /// This is a version of [`add_function`] that accepts a FnMut argument.
    ///
    /// [`add_function`]: #method.add_function
    fn add_function_mut<A, R, F>(&mut self, name: &str, function: F)
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        F: 'static + Send + FnMut(&'lua Lua, A) -> Result<R>;

    /// Add a metamethod which accepts a `&T` as the first parameter.
    ///
    /// # Note
    ///
    /// This can cause an error with certain binary metamethods that can trigger if only the right
    /// side has a metatable. To prevent this, use [`add_meta_function`].
    ///
    /// [`add_meta_function`]: #method.add_meta_function
    fn add_meta_method<A, R, M>(&mut self, meta: MetaMethod, method: M)
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        M: 'static + Send + Fn(&'lua Lua, &T, A) -> Result<R>;

    /// Add a metamethod as a function which accepts a `&mut T` as the first parameter.
    ///
    /// # Note
    ///
    /// This can cause an error with certain binary metamethods that can trigger if only the right
    /// side has a metatable. To prevent this, use [`add_meta_function`].
    ///
    /// [`add_meta_function`]: #method.add_meta_function
    fn add_meta_method_mut<A, R, M>(&mut self, meta: MetaMethod, method: M)
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        M: 'static + Send + FnMut(&'lua Lua, &mut T, A) -> Result<R>;

    /// Add a metamethod which accepts generic arguments.
    ///
    /// Metamethods for binary operators can be triggered if either the left or right argument to
    /// the binary operator has a metatable, so the first argument here is not necessarily a
    /// userdata of type `T`.
    fn add_meta_function<A, R, F>(&mut self, meta: MetaMethod, function: F)
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        F: 'static + Send + Fn(&'lua Lua, A) -> Result<R>;

    /// Add a metamethod as a mutable function which accepts generic arguments.
    ///
    /// This is a version of [`add_meta_function`] that accepts a FnMut argument.
    ///
    /// [`add_meta_function`]: #method.add_meta_function
    fn add_meta_function_mut<A, R, F>(&mut self, meta: MetaMethod, function: F)
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        F: 'static + Send + FnMut(&'lua Lua, A) -> Result<R>;
}

pub(crate) struct StaticUserDataMethods<'lua, T: 'static + UserData> {
    pub(crate) methods: HashMap<StdString, Callback<'lua, 'static>>,
    pub(crate) meta_methods: HashMap<MetaMethod, Callback<'lua, 'static>>,
    pub(crate) _type: PhantomData<T>,
}

impl<'lua, T: 'static + UserData> Default for StaticUserDataMethods<'lua, T> {
    fn default() -> StaticUserDataMethods<'lua, T> {
        StaticUserDataMethods {
            methods: HashMap::new(),
            meta_methods: HashMap::new(),
            _type: PhantomData,
        }
    }
}

impl<'lua, T: 'static + UserData> UserDataMethods<'lua, T> for StaticUserDataMethods<'lua, T> {
    fn add_method<A, R, M>(&mut self, name: &str, method: M)
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        M: 'static + Send + Fn(&'lua Lua, &T, A) -> Result<R>,
    {
        self.methods
            .insert(name.to_owned(), Self::box_method(method));
    }

    fn add_method_mut<A, R, M>(&mut self, name: &str, method: M)
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        M: 'static + Send + FnMut(&'lua Lua, &mut T, A) -> Result<R>,
    {
        self.methods
            .insert(name.to_owned(), Self::box_method_mut(method));
    }

    fn add_function<A, R, F>(&mut self, name: &str, function: F)
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        F: 'static + Send + Fn(&'lua Lua, A) -> Result<R>,
    {
        self.methods
            .insert(name.to_owned(), Self::box_function(function));
    }

    fn add_function_mut<A, R, F>(&mut self, name: &str, function: F)
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        F: 'static + Send + FnMut(&'lua Lua, A) -> Result<R>,
    {
        self.methods
            .insert(name.to_owned(), Self::box_function_mut(function));
    }

    fn add_meta_method<A, R, M>(&mut self, meta: MetaMethod, method: M)
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        M: 'static + Send + Fn(&'lua Lua, &T, A) -> Result<R>,
    {
        self.meta_methods.insert(meta, Self::box_method(method));
    }

    fn add_meta_method_mut<A, R, M>(&mut self, meta: MetaMethod, method: M)
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        M: 'static + Send + FnMut(&'lua Lua, &mut T, A) -> Result<R>,
    {
        self.meta_methods.insert(meta, Self::box_method_mut(method));
    }

    fn add_meta_function<A, R, F>(&mut self, meta: MetaMethod, function: F)
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        F: 'static + Send + Fn(&'lua Lua, A) -> Result<R>,
    {
        self.meta_methods.insert(meta, Self::box_function(function));
    }

    fn add_meta_function_mut<A, R, F>(&mut self, meta: MetaMethod, function: F)
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        F: 'static + Send + FnMut(&'lua Lua, A) -> Result<R>,
    {
        self.meta_methods
            .insert(meta, Self::box_function_mut(function));
    }
}

impl<'lua, T: 'static + UserData> StaticUserDataMethods<'lua, T> {
    fn box_method<A, R, M>(method: M) -> Callback<'lua, 'static>
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        M: 'static + Send + Fn(&'lua Lua, &T, A) -> Result<R>,
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

    fn box_method_mut<A, R, M>(method: M) -> Callback<'lua, 'static>
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        M: 'static + Send + FnMut(&'lua Lua, &mut T, A) -> Result<R>,
    {
        let method = RefCell::new(method);
        Box::new(move |lua, mut args| {
            if let Some(front) = args.pop_front() {
                let userdata = AnyUserData::from_lua(front, lua)?;
                let mut userdata = userdata.borrow_mut::<T>()?;
                let mut method = method
                    .try_borrow_mut()
                    .map_err(|_| Error::RecursiveMutCallback)?;
                (&mut *method)(lua, &mut userdata, A::from_lua_multi(args, lua)?)?.to_lua_multi(lua)
            } else {
                Err(Error::FromLuaConversionError {
                    from: "missing argument",
                    to: "userdata",
                    message: None,
                })
            }
        })
    }

    fn box_function<A, R, F>(function: F) -> Callback<'lua, 'static>
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        F: 'static + Send + Fn(&'lua Lua, A) -> Result<R>,
    {
        Box::new(move |lua, args| function(lua, A::from_lua_multi(args, lua)?)?.to_lua_multi(lua))
    }

    fn box_function_mut<A, R, F>(function: F) -> Callback<'lua, 'static>
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        F: 'static + Send + FnMut(&'lua Lua, A) -> Result<R>,
    {
        let function = RefCell::new(function);
        Box::new(move |lua, args| {
            let function = &mut *function
                .try_borrow_mut()
                .map_err(|_| Error::RecursiveMutCallback)?;
            function(lua, A::from_lua_multi(args, lua)?)?.to_lua_multi(lua)
        })
    }
}

pub(crate) enum NonStaticMethod<'lua, T> {
    Method(Box<Fn(&'lua Lua, &T, MultiValue<'lua>) -> Result<MultiValue<'lua>>>),
    MethodMut(Box<FnMut(&'lua Lua, &mut T, MultiValue<'lua>) -> Result<MultiValue<'lua>>>),
    Function(Box<Fn(&'lua Lua, MultiValue<'lua>) -> Result<MultiValue<'lua>>>),
    FunctionMut(Box<FnMut(&'lua Lua, MultiValue<'lua>) -> Result<MultiValue<'lua>>>),
}

pub(crate) struct NonStaticUserDataMethods<'lua, T: UserData> {
    pub(crate) methods: HashMap<StdString, NonStaticMethod<'lua, T>>,
    pub(crate) meta_methods: HashMap<MetaMethod, NonStaticMethod<'lua, T>>,
}

impl<'lua, T: UserData> Default for NonStaticUserDataMethods<'lua, T> {
    fn default() -> NonStaticUserDataMethods<'lua, T> {
        NonStaticUserDataMethods {
            methods: HashMap::new(),
            meta_methods: HashMap::new(),
        }
    }
}

impl<'lua, T: UserData> UserDataMethods<'lua, T> for NonStaticUserDataMethods<'lua, T> {
    fn add_method<A, R, M>(&mut self, name: &str, method: M)
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        M: 'static + Send + Fn(&'lua Lua, &T, A) -> Result<R>,
    {
        self.methods.insert(
            name.to_owned(),
            NonStaticMethod::Method(Box::new(move |lua, ud, args| {
                method(lua, ud, A::from_lua_multi(args, lua)?)?.to_lua_multi(lua)
            })),
        );
    }

    fn add_method_mut<A, R, M>(&mut self, name: &str, mut method: M)
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        M: 'static + Send + FnMut(&'lua Lua, &mut T, A) -> Result<R>,
    {
        self.methods.insert(
            name.to_owned(),
            NonStaticMethod::MethodMut(Box::new(move |lua, ud, args| {
                method(lua, ud, A::from_lua_multi(args, lua)?)?.to_lua_multi(lua)
            })),
        );
    }

    fn add_function<A, R, F>(&mut self, name: &str, function: F)
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        F: 'static + Send + Fn(&'lua Lua, A) -> Result<R>,
    {
        self.methods.insert(
            name.to_owned(),
            NonStaticMethod::Function(Box::new(move |lua, args| {
                function(lua, A::from_lua_multi(args, lua)?)?.to_lua_multi(lua)
            })),
        );
    }

    fn add_function_mut<A, R, F>(&mut self, name: &str, mut function: F)
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        F: 'static + Send + FnMut(&'lua Lua, A) -> Result<R>,
    {
        self.methods.insert(
            name.to_owned(),
            NonStaticMethod::FunctionMut(Box::new(move |lua, args| {
                function(lua, A::from_lua_multi(args, lua)?)?.to_lua_multi(lua)
            })),
        );
    }

    fn add_meta_method<A, R, M>(&mut self, meta: MetaMethod, method: M)
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        M: 'static + Send + Fn(&'lua Lua, &T, A) -> Result<R>,
    {
        self.meta_methods.insert(
            meta,
            NonStaticMethod::Method(Box::new(move |lua, ud, args| {
                method(lua, ud, A::from_lua_multi(args, lua)?)?.to_lua_multi(lua)
            })),
        );
    }

    fn add_meta_method_mut<A, R, M>(&mut self, meta: MetaMethod, mut method: M)
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        M: 'static + Send + FnMut(&'lua Lua, &mut T, A) -> Result<R>,
    {
        self.meta_methods.insert(
            meta,
            NonStaticMethod::MethodMut(Box::new(move |lua, ud, args| {
                method(lua, ud, A::from_lua_multi(args, lua)?)?.to_lua_multi(lua)
            })),
        );
    }

    fn add_meta_function<A, R, F>(&mut self, meta: MetaMethod, function: F)
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        F: 'static + Send + Fn(&'lua Lua, A) -> Result<R>,
    {
        self.meta_methods.insert(
            meta,
            NonStaticMethod::Function(Box::new(move |lua, args| {
                function(lua, A::from_lua_multi(args, lua)?)?.to_lua_multi(lua)
            })),
        );
    }

    fn add_meta_function_mut<A, R, F>(&mut self, meta: MetaMethod, mut function: F)
    where
        A: FromLuaMulti<'lua>,
        R: ToLuaMulti<'lua>,
        F: 'static + Send + FnMut(&'lua Lua, A) -> Result<R>,
    {
        self.meta_methods.insert(
            meta,
            NonStaticMethod::FunctionMut(Box::new(move |lua, args| {
                function(lua, A::from_lua_multi(args, lua)?)?.to_lua_multi(lua)
            })),
        );
    }
}
