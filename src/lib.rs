#[cfg(test)]
#[macro_use]
extern crate hlist_macro;

#[cfg(not(test))]
extern crate hlist_macro;

#[macro_use]
extern crate error_chain;

pub mod ffi;
mod util;
mod error;
mod lua;
mod conversion;
mod multi;

#[cfg(test)]
mod tests;

pub use error::*;
pub use lua::*;
pub use multi::*;
