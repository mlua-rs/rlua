#![allow(non_snake_case)]

use std::mem;
use std::os::raw::{c_char, c_int, c_void};

use ffi;
use error::Result;
use util::pop_error;

// Uses 1 stack space.
pub unsafe fn lua_newtable(state: *mut ffi::lua_State) -> Result<()> {
    let r = ffi::plua_newtable(state);
    if r == ffi::LUA_OK {
        Ok(())
    } else {
        Err(pop_error(state, r))
    }
}

// Uses 2 stack spaces.
pub unsafe fn luaL_len(state: *mut ffi::lua_State, index: c_int) -> Result<ffi::lua_Integer> {
    let mut len = mem::uninitialized();
    let r = ffi::pluaL_len(state, index, &mut len);
    if r == ffi::LUA_OK {
        Ok(len)
    } else {
        Err(pop_error(state, r))
    }
}

// Uses 3 stack spaces.
pub unsafe fn lua_geti(
    state: *mut ffi::lua_State,
    index: c_int,
    i: ffi::lua_Integer,
) -> Result<c_int> {
    let r = ffi::plua_geti(state, index, i);
    if r == ffi::LUA_OK {
        Ok(ffi::lua_type(state, -1))
    } else {
        Err(pop_error(state, r))
    }
}

// Uses 2 stack spaces.
pub unsafe fn lua_gettable(state: *mut ffi::lua_State, index: c_int) -> Result<c_int> {
    let r = ffi::plua_gettable(state, index);
    if r == ffi::LUA_OK {
        Ok(ffi::lua_type(state, -1))
    } else {
        Err(pop_error(state, r))
    }
}

// Uses 1 stack space.
pub unsafe fn lua_newthread(state: *mut ffi::lua_State) -> Result<*mut ffi::lua_State> {
    let mut thread = mem::uninitialized();
    let r = ffi::plua_newthread(state, &mut thread);
    if r == ffi::LUA_OK {
        Ok(thread)
    } else {
        Err(pop_error(state, r))
    }
}

// Uses 2 stack spaces.
pub unsafe fn lua_newuserdata(state: *mut ffi::lua_State, size: usize) -> Result<*mut c_void> {
    let mut ud = mem::uninitialized();
    let r = ffi::plua_newuserdata(state, size, &mut ud);
    if r == ffi::LUA_OK {
        Ok(ud)
    } else {
        Err(pop_error(state, r))
    }
}

// Uses 2 stack spaces.
pub unsafe fn lua_next(state: *mut ffi::lua_State, index: c_int) -> Result<c_int> {
    let mut res = mem::uninitialized();
    let r = ffi::plua_next(state, index, &mut res);
    if r == ffi::LUA_OK {
        Ok(res)
    } else {
        Err(pop_error(state, r))
    }
}

// Uses 2 stack spaces.
pub unsafe fn lua_pushcclosure(
    state: *mut ffi::lua_State,
    function: ffi::lua_CFunction,
    n: c_int,
) -> Result<()> {
    let r = ffi::plua_pushcclosure(state, function, n);
    if r == ffi::LUA_OK {
        Ok(())
    } else {
        Err(pop_error(state, r))
    }
}

// Uses 3 stack spaces
pub unsafe fn lua_pushlstring(
    state: *mut ffi::lua_State,
    s: *const c_char,
    len: usize,
) -> Result<()> {
    let r = ffi::plua_pushlstring(state, s, len);
    if r == ffi::LUA_OK {
        Ok(())
    } else {
        Err(pop_error(state, r))
    }
}

// Uses 2 stack spaces
pub unsafe fn lua_pushstring(state: *mut ffi::lua_State, s: *const c_char) -> Result<()> {
    let r = ffi::plua_pushstring(state, s);
    if r == ffi::LUA_OK {
        Ok(())
    } else {
        Err(pop_error(state, r))
    }
}

// Uses 4 stack spaces
pub unsafe fn lua_rawset(state: *mut ffi::lua_State, index: c_int) -> Result<()> {
    let r = ffi::plua_rawset(state, index);
    if r == ffi::LUA_OK {
        Ok(())
    } else {
        Err(pop_error(state, r))
    }
}

// Uses 4 stack spaces
pub unsafe fn lua_settable(state: *mut ffi::lua_State, index: c_int) -> Result<()> {
    let r = ffi::plua_settable(state, index);
    if r == ffi::LUA_OK {
        Ok(())
    } else {
        Err(pop_error(state, r))
    }
}

// Uses 3 stack spaces
pub unsafe fn lua_tostring(state: *mut ffi::lua_State, index: c_int) -> Result<*const c_char> {
    let mut res = mem::uninitialized();
    let r = ffi::plua_tostring(state, index, &mut res);
    if r == ffi::LUA_OK {
        Ok(res)
    } else {
        Err(pop_error(state, r))
    }
}

// uses n + 2 stack spaces.
pub unsafe fn lua_pushrclosure(
    state: *mut ffi::lua_State,
    function: ffi::RustCallback,
    n: c_int,
) -> Result<()> {
    let r = ffi::plua_pushrclosure(state, function, n);
    if r == ffi::LUA_OK {
        Ok(())
    } else {
        Err(pop_error(state, r))
    }
}

// uses n + 2 stack spaces.
pub unsafe fn lua_pushrfunction(
    state: *mut ffi::lua_State,
    function: ffi::RustCallback,
) -> Result<()> {
    lua_pushrclosure(state, function, 0)
}
