use std::os::raw::c_int;
use std::ptr;

use libc::c_void;

use crate::error::{Error, Result};
use crate::ffi;
use crate::types::LuaRef;
use crate::util::{
    assert_stack, check_stack, dump, error_traceback, pop_error, protect_lua_closure, rotate,
    StackGuard,
};
use crate::value::{FromLuaMulti, MultiValue, ToLuaMulti};

/// Handle to an internal Lua function.
#[derive(Clone, Debug)]
pub struct Function<'lua>(pub(crate) LuaRef<'lua>);

impl<'lua> Function<'lua> {
    /// Calls the function, passing `args` as function arguments.
    ///
    /// The function's return values are converted to the generic type `R`.
    ///
    /// # Examples
    ///
    /// Call Lua's built-in `tostring` function:
    ///
    /// ```
    /// # use rlua::{Lua, Function, Result};
    /// # fn main() -> Result<()> {
    /// # Lua::new().context(|lua_context| {
    /// let globals = lua_context.globals();
    ///
    /// let tostring: Function = globals.get("tostring")?;
    ///
    /// assert_eq!(tostring.call::<_, String>(123)?, "123");
    ///
    /// # Ok(())
    /// # })
    /// # }
    /// ```
    ///
    /// Call a function with multiple arguments:
    ///
    /// ```
    /// # use rlua::{Lua, Function, Result};
    /// # fn main() -> Result<()> {
    /// # Lua::new().context(|lua_context| {
    /// let sum: Function = lua_context.load(
    ///     r#"
    ///         function(a, b)
    ///             return a + b
    ///         end
    ///     "#).eval()?;
    ///
    /// assert_eq!(sum.call::<_, u32>((3, 4))?, 3 + 4);
    ///
    /// # Ok(())
    /// # })
    /// # }
    /// ```
    pub fn call<A: ToLuaMulti<'lua>, R: FromLuaMulti<'lua>>(&self, args: A) -> Result<R> {
        let lua = self.0.lua;

        let args = args.to_lua_multi(lua)?;
        let nargs = args.len() as c_int;

        let results = unsafe {
            let _sg = StackGuard::new(lua.state);
            check_stack(lua.state, nargs + 3)?;

            ffi::lua_pushcfunction(lua.state, Some(error_traceback));
            let stack_start = ffi::lua_gettop(lua.state);
            lua.push_ref(&self.0);
            for arg in args {
                lua.push_value(arg)?;
            }
            let ret = ffi::lua_pcall(lua.state, nargs, ffi::LUA_MULTRET, stack_start);
            if ret != ffi::LUA_OK as i32 {
                return Err(pop_error(lua.state, ret));
            }
            let nresults = ffi::lua_gettop(lua.state) - stack_start;
            let mut results = MultiValue::new();
            assert_stack(lua.state, 2);
            for _ in 0..nresults {
                results.push_front(lua.pop_value());
            }
            ffi::lua_pop(lua.state, 1);
            results
        };
        R::from_lua_multi(results, lua)
    }

    /// Returns a function that, when called, calls `self`, passing `args` as the first set of
    /// arguments.
    ///
    /// If any arguments are passed to the returned function, they will be passed after `args`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rlua::{Lua, Function, Result};
    /// # fn main() -> Result<()> {
    /// # Lua::new().context(|lua_context| {
    /// let sum: Function = lua_context.load(r#"
    ///     function(a, b)
    ///         return a + b
    ///     end
    /// "#).eval()?;
    ///
    /// let bound_a = sum.bind(1)?;
    /// assert_eq!(bound_a.call::<_, u32>(2)?, 1 + 2);
    ///
    /// let bound_a_and_b = sum.bind(13)?.bind(57)?;
    /// assert_eq!(bound_a_and_b.call::<_, u32>(())?, 13 + 57);
    ///
    /// # Ok(())
    /// # })
    /// # }
    /// ```
    pub fn bind<A: ToLuaMulti<'lua>>(&self, args: A) -> Result<Function<'lua>> {
        unsafe extern "C" fn bind_call_impl(state: *mut ffi::lua_State) -> c_int {
            let nargs = ffi::lua_gettop(state);
            let nbinds = ffi::lua_tointeger(state, ffi::lua_upvalueindex(2)) as c_int;
            ffi::luaL_checkstack(state, nbinds + 2, ptr::null());

            ffi::lua_settop(state, nargs + nbinds + 1);
            rotate(state, -(nargs + nbinds + 1), nbinds + 1);

            ffi::lua_pushvalue(state, ffi::lua_upvalueindex(1));
            ffi::lua_replace(state, 1);

            for i in 0..nbinds {
                ffi::lua_pushvalue(state, ffi::lua_upvalueindex(i + 3));
                ffi::lua_replace(state, i + 2);
            }

            ffi::lua_call(state, nargs + nbinds, ffi::LUA_MULTRET);
            ffi::lua_gettop(state)
        }

        let lua = self.0.lua;

        let args = args.to_lua_multi(lua)?;
        let nargs = args.len() as c_int;

        if nargs + 2 > ffi::LUA_MAX_UPVALUES {
            return Err(Error::BindError);
        }

        unsafe {
            let _sg = StackGuard::new(lua.state);
            check_stack(lua.state, nargs + 5)?;
            lua.push_ref(&self.0);
            ffi::lua_pushinteger(lua.state, nargs as ffi::lua_Integer);
            for arg in args {
                lua.push_value(arg)?;
            }

            protect_lua_closure(lua.state, nargs + 2, 1, |state| {
                ffi::lua_pushcclosure(state, Some(bind_call_impl), nargs + 2);
            })?;

            Ok(Function(lua.pop_ref()))
        }
    }

    /// Dumps the compiled representation of the function into a binary blob,
    /// which can later be loaded using the unsafe Chunk::into_function_allow_binary().
    ///
    /// # Examples
    ///
    /// ```
    /// # use rlua::{Lua, Function, Result};
    /// # fn main() -> Result<()> {
    /// # Lua::new().context(|lua_context| {
    /// let add2: Function = lua_context.load(r#"
    ///     function(a)
    ///         return a + 2
    ///     end
    /// "#).eval()?;
    ///
    /// let dumped = add2.dump()?;
    ///
    /// let reloaded = unsafe {
    ///     lua_context.load(&dumped)
    ///                .into_function_allow_binary()?
    /// };
    /// assert_eq!(reloaded.call::<_, u32>(7)?, 7+2);
    ///
    /// # Ok(())
    /// # })
    /// # }
    /// ```
    pub fn dump(&self) -> Result<Vec<u8>> {
        unsafe extern "C" fn writer(
            _state: *mut ffi::lua_State,
            p: *const c_void,
            sz: usize,
            ud: *mut c_void,
        ) -> c_int {
            let input_slice = std::slice::from_raw_parts(p as *const u8, sz);
            let vec = &mut *(ud as *mut Vec<u8>);
            vec.extend_from_slice(input_slice);
            0
        }
        let lua = self.0.lua;
        let mut bytes = Vec::new();
        unsafe {
            let _sg = StackGuard::new(lua.state);
            check_stack(lua.state, 1)?;
            let bytes_ptr = &mut bytes as *mut _;
            protect_lua_closure(lua.state, 0, 0, |state| {
                lua.push_ref(&self.0);
                let dump_result = dump(state, Some(writer), bytes_ptr as *mut c_void, 0);
                // It can only return an error from our writer.
                debug_assert_eq!(dump_result, 0);
            })?;
        }
        Ok(bytes)
    }
}
