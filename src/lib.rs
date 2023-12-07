//! # High-level bindings to Lua
//!
//! The `rlua` crate provides safe high-level bindings to the [Lua programming language].
//!
//! # The `Lua` object
//!
//! The main type exported by this library is the [`Lua`] struct. In addition to methods for
//! [executing] Lua chunks or [evaluating] Lua expressions, it provides methods for creating Lua
//! values and accessing the table of [globals].
//!
//! # Converting data
//!
//! The [`ToLua`] and [`FromLua`] traits allow conversion from Rust types to Lua values and vice
//! versa. They are implemented for many data structures found in Rust's standard library.
//!
//! For more general conversions, the [`ToLuaMulti`] and [`FromLuaMulti`] traits allow converting
//! between Rust types and *any number* of Lua values.
//!
//! Most code in `rlua` is generic over implementors of those traits, so in most places the normal
//! Rust data structures are accepted without having to write any boilerplate.
//!
//! # Custom Userdata
//!
//! The [`UserData`] trait can be implemented by user-defined types to make them available to Lua.
//! Methods and operators to be used from Lua can be added using the [`UserDataMethods`] API.
//!
//! [Lua programming language]: https://www.lua.org/
//! [`Lua`]: struct.Lua.html
//! [executing]: struct.Context.html#method.exec
//! [evaluating]: struct.Context.html#method.eval
//! [globals]: struct.Context.html#method.globals
//! [`ToLua`]: trait.ToLua.html
//! [`FromLua`]: trait.FromLua.html
//! [`ToLuaMulti`]: trait.ToLuaMulti.html
//! [`FromLuaMulti`]: trait.FromLuaMulti.html
//! [`UserData`]: trait.UserData.html
//! [`UserDataMethods`]: trait.UserDataMethods.html

// Deny warnings inside doc tests / examples. When this isn't present, rustdoc doesn't show *any*
// warnings at all.
#![doc(test(attr(deny(warnings))))]

#[macro_use]
mod macros;

#[cfg(rlua_lua51)]
extern crate rlua_lua51_sys as ffi;
#[cfg(rlua_lua53)]
extern crate rlua_lua53_sys as ffi;
#[cfg(rlua_lua54)]
extern crate rlua_lua54_sys as ffi;
mod context;
mod conversion;
mod error;
mod function;
mod hook;
mod lua;
mod markers;
mod multi;
mod scope;
mod string;
mod table;
mod thread;
mod types;
mod userdata;
mod util;
mod value;

pub use crate::context::{Chunk, Context};
pub use crate::error::{Error, ExternalError, ExternalResult, Result};
pub use crate::function::Function;
pub use crate::hook::{Debug, DebugNames, DebugSource, DebugStack, HookTriggers};
pub use crate::lua::{InitFlags, Lua, StdLib};
pub use crate::multi::{Fallible, Variadic};
pub use crate::scope::Scope;
pub use crate::string::String;
pub use crate::table::{Table, TablePairs, TableSequence};
pub use crate::thread::{Thread, ThreadStatus};
pub use crate::types::{Integer, LightUserData, Number, RegistryKey};
pub use crate::userdata::{AnyUserData, MetaMethod, UserData, UserDataMethods};
pub use crate::value::{FromLua, FromLuaMulti, MultiValue, Nil, ToLua, ToLuaMulti, Value};

pub mod prelude;
