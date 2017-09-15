use std::{slice, str};

use ffi;
use lua::LuaRef;
use error::{Error, Result};
use util::{check_stack, stack_guard};

/// Handle to an internal Lua string.
///
/// Unlike Rust strings, Lua strings may not be valid UTF-8.
#[derive(Clone, Debug)]
pub struct String<'lua>(pub(crate) LuaRef<'lua>);

impl<'lua> String<'lua> {
    /// Get a `&str` slice if the Lua string is valid UTF-8.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate rlua;
    /// # use rlua::{Lua, String, Result};
    /// # fn try_main() -> Result<()> {
    /// let lua = Lua::new();
    /// let globals = lua.globals();
    ///
    /// let version: String = globals.get("_VERSION")?;
    /// assert!(version.to_str().unwrap().contains("Lua"));
    ///
    /// let non_utf8: String = lua.eval(r#"  "test\xff"  "#, None)?;
    /// assert!(non_utf8.to_str().is_err());
    /// # Ok(())
    /// # }
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    pub fn to_str(&self) -> Result<&str> {
        str::from_utf8(self.as_bytes()).map_err(|e| {
            Error::FromLuaConversionError {
                from: "string",
                to: "&str",
                message: Some(e.to_string()),
            }
        })
    }

    /// Get the bytes that make up this string.
    ///
    /// The returned slice will not contain the terminating null byte, but will contain any null
    /// bytes embedded into the Lua string.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate rlua;
    /// # use rlua::{Lua, String};
    /// # fn main() {
    /// let lua = Lua::new();
    ///
    /// let non_utf8: String = lua.eval(r#"  "test\xff"  "#, None).unwrap();
    /// assert!(non_utf8.to_str().is_err());    // oh no :(
    /// assert_eq!(non_utf8.as_bytes(), &b"test\xff"[..]);
    /// # }
    /// ```
    pub fn as_bytes(&self) -> &[u8] {
        let lua = self.0.lua;
        unsafe {
            stack_guard(lua.state, 0, || {
                check_stack(lua.state, 1);
                lua.push_ref(lua.state, &self.0);
                assert_eq!(ffi::lua_type(lua.state, -1), ffi::LUA_TSTRING);

                let mut size = 0;
                let data = ffi::lua_tolstring(lua.state, -1, &mut size);

                ffi::lua_pop(lua.state, 1);
                slice::from_raw_parts(data as *const u8, size)
            })
        }
    }
}
