use std::{slice, str};

use ffi;
use error::{Error, Result};
use util::{check_stack, stack_guard};
use types::LuaRef;

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
    /// The returned slice will not contain the terminating nul byte, but will contain any nul
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
        let nulled = self.as_bytes_with_nul();
        &nulled[..nulled.len() - 1]
    }

    /// Get the bytes that make up this string, including the trailing nul byte.
    pub fn as_bytes_with_nul(&self) -> &[u8] {
        let lua = self.0.lua;
        unsafe {
            stack_guard(lua.state, 0, || {
                check_stack(lua.state, 1);
                lua.push_ref(lua.state, &self.0);
                assert_eq!(ffi::lua_type(lua.state, -1), ffi::LUA_TSTRING);

                let mut size = 0;
                let data = ffi::lua_tolstring(lua.state, -1, &mut size);

                ffi::lua_pop(lua.state, 1);
                slice::from_raw_parts(data as *const u8, size + 1)
            })
        }
    }
}

impl<'lua> AsRef<[u8]> for String<'lua> {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

// Lua strings are basically &[u8] slices, so implement PartialEq for anything resembling that.
//
// This makes our `String` comparable with `Vec<u8>`, `[u8]`, `&str`, `String` and `rlua::String`
// itself.
//
// The only downside is that this disallows a comparison with `Cow<str>`, as that only implements
// `AsRef<str>`, which collides with this impl. Requiring `AsRef<str>` would fix that, but limit us
// in other ways.
impl<'lua, T> PartialEq<T> for String<'lua>
where
    T: AsRef<[u8]>,
{
    fn eq(&self, other: &T) -> bool {
        self.as_bytes() == other.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lua::Lua;

    use std::borrow::Cow;

    fn with_str<F>(s: &str, f: F)
    where
        F: FnOnce(String),
    {
        let lua = Lua::new();
        let string = lua.create_string(s);
        f(string);
    }

    #[test]
    fn compare() {
        // Tests that all comparisons we want to have are usable
        with_str("teststring", |t| assert_eq!(t, "teststring")); // &str
        with_str("teststring", |t| assert_eq!(t, b"teststring")); // &[u8]
        with_str("teststring", |t| assert_eq!(t, b"teststring".to_vec())); // Vec<u8>
        with_str("teststring", |t| assert_eq!(t, "teststring".to_string())); // String
        with_str("teststring", |t| assert_eq!(t, t)); // rlua::String
        with_str("teststring", |t| {
            assert_eq!(t, Cow::from(b"teststring".as_ref()))
        }); // Cow (borrowed)
        with_str("bla", |t| assert_eq!(t, Cow::from(b"bla".to_vec()))); // Cow (owned)
    }

    #[test]
    fn string_views() {
        let lua = Lua::new();
        lua.eval::<()>(
            r#"
                ok = "null bytes are valid utf-8, wh\0 knew?"
                err = "but \xff isn't :("
                empty = ""
            "#,
            None,
        ).unwrap();

        let globals = lua.globals();
        let ok: String = globals.get("ok").unwrap();
        let err: String = globals.get("err").unwrap();
        let empty: String = globals.get("empty").unwrap();

        assert_eq!(
            ok.to_str().unwrap(),
            "null bytes are valid utf-8, wh\0 knew?"
        );
        assert_eq!(
            ok.as_bytes(),
            &b"null bytes are valid utf-8, wh\0 knew?"[..]
        );

        assert!(err.to_str().is_err());
        assert_eq!(err.as_bytes(), &b"but \xff isn't :("[..]);

        assert_eq!(empty.to_str().unwrap(), "");
        assert_eq!(empty.as_bytes_with_nul(), &[0]);
        assert_eq!(empty.as_bytes(), &[]);
    }
}
