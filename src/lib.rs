#[cfg_attr(test, macro_use)]
extern crate hlist_macro;

pub mod ffi;
#[macro_use]
mod util;
mod error;
mod lua;
mod conversion;
mod multi;

#[cfg(test)]
mod tests;

pub use error::{Error, Result, ExternalError, ExternalResult};
pub use lua::{Value, Nil, ToLua, FromLua, MultiValue, ToLuaMulti, FromLuaMulti, Integer, Number,
              LightUserData, String, Table, TablePairs, TableSequence, Function, ThreadStatus,
              Thread, MetaMethod, UserDataMethods, UserData, AnyUserData, Lua};
pub use multi::Variadic;

pub mod prelude;
