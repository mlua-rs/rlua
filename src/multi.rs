use std::iter::FromIterator;
use std::ops::{Deref, DerefMut};
use std::result::Result as StdResult;

use crate::context::Context;
use crate::error::Result;
use crate::value::{FromLua, FromLuaMulti, MultiValue, Nil, ToLua, ToLuaMulti};

/// Result is convertible to `MultiValue` following the common Lua idiom of returning the result
/// on success, or in the case of an error, returning `nil` and an error message.
impl<'lua, T: ToLua<'lua>, E: ToLua<'lua>> ToLuaMulti<'lua> for StdResult<T, E> {
    fn to_lua_multi(self, lua: Context<'lua>) -> Result<MultiValue<'lua>> {
        let mut result = MultiValue::new();

        match self {
            Ok(v) => result.push_front(v.to_lua(lua)?),
            Err(e) => {
                result.push_front(e.to_lua(lua)?);
                result.push_front(Nil);
            }
        }

        Ok(result)
    }
}

impl<'lua, T: ToLua<'lua>> ToLuaMulti<'lua> for T {
    fn to_lua_multi(self, lua: Context<'lua>) -> Result<MultiValue<'lua>> {
        let mut v = MultiValue::new();
        v.push_front(self.to_lua(lua)?);
        Ok(v)
    }
}

impl<'lua, T: FromLua<'lua>> FromLuaMulti<'lua> for T {
    fn from_lua_multi(
        mut values: MultiValue<'lua>,
        lua: Context<'lua>,
        consumed: &mut usize,
    ) -> Result<Self> {
        match values.pop_front() {
            Some(it) => {
                *consumed += 1;
                Ok(T::from_lua(it, lua)?)
            }
            None => Ok(T::from_lua(Nil, lua)?),
        }
    }
}

impl<'lua> ToLuaMulti<'lua> for MultiValue<'lua> {
    fn to_lua_multi(self, _: Context<'lua>) -> Result<MultiValue<'lua>> {
        Ok(self)
    }
}

impl<'lua> FromLuaMulti<'lua> for MultiValue<'lua> {
    fn from_lua_multi(
        values: MultiValue<'lua>,
        _: Context<'lua>,
        consumed: &mut usize,
    ) -> Result<Self> {
        *consumed += values.len();
        Ok(values)
    }
}

/// Wraps a variable number of `T`s.
///
/// Can be used to work with variadic functions more easily. Using this type as the last argument of
/// a Rust callback will accept any number of arguments from Lua and convert them to the type `T`
/// using [`FromLua`]. `Variadic<T>` can also be returned from a callback, returning a variable
/// number of values to Lua.
///
/// The [`MultiValue`] type is equivalent to `Variadic<Value>`.
///
/// # Examples
///
/// ```
/// # use rlua::{Lua, Variadic, Result};
/// # fn main() -> Result<()> {
/// # Lua::new().context(|lua_context| {
/// let add = lua_context.create_function(|_, vals: Variadic<f64>| -> Result<f64> {
///     Ok(vals.iter().sum())
/// }).unwrap();
/// lua_context.globals().set("add", add)?;
/// assert_eq!(lua_context.load("add(3, 2, 5)").eval::<f32>()?, 10.0);
/// # Ok(())
/// # })
/// # }
/// ```
///
/// [`FromLua`]: trait.FromLua.html
/// [`MultiValue`]: struct.MultiValue.html
#[derive(Debug, Clone)]
pub struct Variadic<T>(Vec<T>);

impl<T> Variadic<T> {
    /// Creates an empty `Variadic` wrapper containing no values.
    pub fn new() -> Variadic<T> {
        Variadic(Vec::new())
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl<T> Default for Variadic<T> {
    fn default() -> Variadic<T> {
        Variadic::new()
    }
}

impl<T> FromIterator<T> for Variadic<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Variadic(Vec::from_iter(iter))
    }
}

impl<T> IntoIterator for Variadic<T> {
    type Item = T;
    type IntoIter = <Vec<T> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<T> Deref for Variadic<T> {
    type Target = Vec<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Variadic<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'lua, T: ToLua<'lua>> ToLuaMulti<'lua> for Variadic<T> {
    fn to_lua_multi(self, lua: Context<'lua>) -> Result<MultiValue<'lua>> {
        self.0.into_iter().map(|e| e.to_lua(lua)).collect()
    }
}

impl<'lua, T: FromLuaMulti<'lua>> FromLuaMulti<'lua> for Variadic<T> {
    fn from_lua_multi(
        mut values: MultiValue<'lua>,
        lua: Context<'lua>,
        total: &mut usize,
    ) -> Result<Self> {
        let mut result = Vec::new();
        while values.len() > 0 {
            let mut consumed = 0;
            if let Ok(it) = T::from_lua_multi(values.clone(), lua, &mut consumed) {
                result.push(it);
                values.drop_front(consumed);
                *total += consumed;
            } else {
                break;
            }
        }
        Ok(Variadic(result))
    }
}

/// Wrapper for arguments that allowed to fail during conversion.
///
/// Note that failing includes recieving a `nil` value if the type isn't
/// convertible from `nil`. That is, this wrapper allows _skipping_ arguments in
/// called functions. Capturing nil for skippable arguments can be done through
/// `Fallible<Option<T>>` as it will have a value of `Some(None)` when
/// converting from `nil`.
///
/// In case where `nil` argument is expected (must be specified from the
/// script), `Option` should be used instead of `Fallible`.
///
/// If conversion is successful, the value will be `Some(T)`.
///
/// Conversely, if conversion fails, the value will be `None`, and `consumed`
/// argument counter will stay unchanged.
pub struct Fallible<T>(Option<T>);

impl<T> Fallible<T> {
    /// Returns inner `Option<T>`.
    pub fn into_option(self) -> Option<T> {
        self.0
    }

    /// Maps fallible type using provided `mapping` function into another
    /// `Option`.
    pub fn map<R, F: Fn(T) -> R>(self, mapping: F) -> Option<R> {
        self.0.map(mapping)
    }

    /// Unwraps fallible type or panics if conversion failed.
    pub fn unwrap(self) -> T {
        self.0.unwrap()
    }
    /// Unwraps fallible type or returns `value` if conversion failed.
    pub fn unwrap_or(self, value: T) -> T {
        self.0.unwrap_or(value)
    }
    /// Unwraps fallible type or returns a return value of `init` if conversion
    /// failed.
    pub fn unwrap_or_else<F: Fn() -> T>(self, f: F) -> T {
        self.0.unwrap_or_else(f)
    }
    /// Unwraps fallible type or returns the default value if conversion failed.
    pub fn unwrap_or_default(self) -> T
    where
        T: Default,
    {
        self.0.unwrap_or_else(T::default)
    }
    /// Retuns `other` `Option` if this argument conversion failed.
    pub fn or(self, other: Option<T>) -> Option<T> {
        self.0.or(other)
    }
    /// Retuns `Option` value returned by `f` if this argument conversion
    /// failed.
    pub fn or_else<F: Fn() -> Option<T>>(self, f: F) -> Option<T> {
        self.0.or_else(f)
    }
}

impl<T> Deref for Fallible<T> {
    type Target = Option<T>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<T> DerefMut for Fallible<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'lua, T: FromLuaMulti<'lua>> FromLuaMulti<'lua> for Fallible<T> {
    fn from_lua_multi(
        values: MultiValue<'lua>,
        lua: Context<'lua>,
        consumed: &mut usize,
    ) -> Result<Self> {
        match T::from_lua_multi(values, lua, consumed) {
            Ok(it) => {
                *consumed += 1;
                Ok(Fallible(Some(it)))
            }
            Err(_) => Ok(Fallible(None)),
        }
    }
}

macro_rules! impl_tuple {
    ($($name:ident)*) => (
        impl<'lua, $($name),*> ToLuaMulti<'lua> for ($($name,)*)
            where $($name: ToLuaMulti<'lua>),*
        {
            #[allow(unused_variables)]
            #[allow(unused_mut)]
            #[allow(non_snake_case)]
            fn to_lua_multi(self, lua: Context<'lua>) -> Result<MultiValue<'lua>> {
                let ($($name,)*) = self;

                let mut results = MultiValue::new();
                push_reverse!(results, $($name.to_lua_multi(lua)?,)*);
                Ok(results)
            }
        }

        impl<'lua, $($name),*> FromLuaMulti<'lua> for ($($name,)*)
            where $($name: FromLuaMulti<'lua>),*
        {
            #[allow(unused_variables)]
            #[allow(unused_mut)]
            #[allow(non_snake_case)]
            fn from_lua_multi(mut values: MultiValue<'lua>, lua: Context<'lua>, total: &mut usize) -> Result<Self> {
                $(
                    let $name = {
                        let mut consumed = 0;
                        let it = match FromLuaMulti::from_lua_multi(values.clone(), lua, &mut consumed) {
                            Ok(it) => it,
                            Err(err) => {
                                return Err(err);
                            }
                        };
                        *total += consumed;
                        values.drop_front(consumed);
                        it
                    };
                )*
                Ok(($($name,)*))
            }
        }
    );
}

macro_rules! push_reverse {
    ($multi_value:expr, $first:expr, $($rest:expr,)*) => (
        push_reverse!($multi_value, $($rest,)*);
        $multi_value.append($first);
    );

    ($multi_value:expr, $first:expr) => (
        $multi_value.append($first);
    );

    ($multi_value:expr,) => ();
}

impl_tuple!();
impl_tuple!(A);
impl_tuple!(A B);
impl_tuple!(A B C);
impl_tuple!(A B C D);
impl_tuple!(A B C D E);
impl_tuple!(A B C D E F);
impl_tuple!(A B C D E F G);
impl_tuple!(A B C D E F G H);
impl_tuple!(A B C D E F G H I);
impl_tuple!(A B C D E F G H I J);
impl_tuple!(A B C D E F G H I J K);
impl_tuple!(A B C D E F G H I J K L);
impl_tuple!(A B C D E F G H I J K L M);
impl_tuple!(A B C D E F G H I J K L M N);
impl_tuple!(A B C D E F G H I J K L M N O);
impl_tuple!(A B C D E F G H I J K L M N O P);
