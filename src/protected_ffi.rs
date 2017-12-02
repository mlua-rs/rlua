#![allow(unused)]

use std::os::raw::{c_char, c_int, c_void};
use std::{mem, ptr};

use ffi;

// Protected version of lua_gettable, uses 3 stack spaces, does not call checkstack.
pub unsafe fn pgettable(
    state: *mut ffi::lua_State,
    index: c_int,
    msgh: c_int,
) -> Result<c_int, c_int> {
    unsafe extern "C" fn gettable(state: *mut ffi::lua_State) -> c_int {
        ffi::lua_gettable(state, -2);
        1
    }

    let table_index = ffi::lua_absindex(state, index);

    ffi::lua_pushcfunction(state, gettable);
    ffi::lua_pushvalue(state, table_index);
    ffi::lua_pushvalue(state, -3);
    ffi::lua_remove(state, -4);

    let ret = ffi::lua_pcall(state, 2, 1, msgh);
    if ret == ffi::LUA_OK {
        Ok(ffi::lua_type(state, -1))
    } else {
        Err(ret)
    }
}

// Protected version of lua_settable, uses 4 stack spaces, does not call checkstack.
pub unsafe fn psettable(
    state: *mut ffi::lua_State,
    index: c_int,
    msgh: c_int,
) -> Result<(), c_int> {
    unsafe extern "C" fn settable(state: *mut ffi::lua_State) -> c_int {
        ffi::lua_settable(state, -3);
        0
    }

    let table_index = ffi::lua_absindex(state, index);

    ffi::lua_pushcfunction(state, settable);
    ffi::lua_pushvalue(state, table_index);
    ffi::lua_pushvalue(state, -4);
    ffi::lua_pushvalue(state, -4);
    ffi::lua_remove(state, -5);
    ffi::lua_remove(state, -5);

    let ret = ffi::lua_pcall(state, 3, 0, msgh);
    if ret == ffi::LUA_OK {
        Ok(())
    } else {
        Err(ret)
    }
}

// Protected version of luaL_len, uses 2 stack spaces, does not call checkstack.
pub unsafe fn plen(
    state: *mut ffi::lua_State,
    index: c_int,
    msgh: c_int,
) -> Result<ffi::lua_Integer, c_int> {
    unsafe extern "C" fn len(state: *mut ffi::lua_State) -> c_int {
        ffi::lua_pushinteger(state, ffi::luaL_len(state, -1));
        1
    }

    let table_index = ffi::lua_absindex(state, index);

    ffi::lua_pushcfunction(state, len);
    ffi::lua_pushvalue(state, table_index);

    let ret = ffi::lua_pcall(state, 1, 1, msgh);
    if ret == ffi::LUA_OK {
        let len = ffi::lua_tointeger(state, -1);
        ffi::lua_pop(state, 1);
        Ok(len)
    } else {
        Err(ret)
    }
}

// Protected version of lua_geti, uses 3 stack spaces, does not call checkstack.
pub unsafe fn pgeti(
    state: *mut ffi::lua_State,
    index: c_int,
    i: ffi::lua_Integer,
    msgh: c_int,
) -> Result<c_int, c_int> {
    unsafe extern "C" fn geti(state: *mut ffi::lua_State) -> c_int {
        let i = ffi::lua_tointeger(state, -1);
        ffi::lua_geti(state, -2, i);
        1
    }

    let table_index = ffi::lua_absindex(state, index);

    ffi::lua_pushcfunction(state, geti);
    ffi::lua_pushvalue(state, table_index);
    ffi::lua_pushinteger(state, i);

    let ret = ffi::lua_pcall(state, 2, 1, msgh);
    if ret == ffi::LUA_OK {
        Ok(ffi::lua_type(state, -1))
    } else {
        Err(ret)
    }
}

// Protected version of lua_next, uses 3 stack spaces, does not call checkstack.
pub unsafe fn pnext(state: *mut ffi::lua_State, index: c_int, msgh: c_int) -> Result<c_int, c_int> {
    unsafe extern "C" fn next(state: *mut ffi::lua_State) -> c_int {
        if ffi::lua_next(state, -2) == 0 {
            0
        } else {
            2
        }
    }

    let table_index = ffi::lua_absindex(state, index);

    ffi::lua_pushcfunction(state, next);
    ffi::lua_pushvalue(state, table_index);
    ffi::lua_pushvalue(state, -3);
    ffi::lua_remove(state, -4);

    let stack_start = ffi::lua_gettop(state) - 3;
    let ret = ffi::lua_pcall(state, 2, ffi::LUA_MULTRET, msgh);
    if ret == ffi::LUA_OK {
        let nresults = ffi::lua_gettop(state) - stack_start;
        if nresults == 0 {
            Ok(0)
        } else {
            Ok(1)
        }
    } else {
        Err(ret)
    }
}

// Protected version of lua_newtable, uses 1 stack space, does not call checkstack.
pub unsafe fn pnewtable(state: *mut ffi::lua_State, msgh: c_int) -> Result<(), c_int> {
    unsafe extern "C" fn newtable(state: *mut ffi::lua_State) -> c_int {
        ffi::lua_newtable(state);
        1
    }

    ffi::lua_pushcfunction(state, newtable);

    let ret = ffi::lua_pcall(state, 0, 1, msgh);
    if ret == ffi::LUA_OK {
        Ok(())
    } else {
        Err(ret)
    }
}

// Protected version of lua_newthread, uses 1 stack space, does not call checkstack.
pub unsafe fn pnewthread(
    state: *mut ffi::lua_State,
    msgh: c_int,
) -> Result<*mut ffi::lua_State, c_int> {
    unsafe extern "C" fn newthread(state: *mut ffi::lua_State) -> c_int {
        ffi::lua_newthread(state);
        1
    }

    ffi::lua_pushcfunction(state, newthread);

    let ret = ffi::lua_pcall(state, 0, 1, msgh);
    if ret == ffi::LUA_OK {
        Ok(ffi::lua_tothread(state, -1))
    } else {
        Err(ret)
    }
}

// Protected version of lua_newuserdata, uses 2 stack spaces, does not call checkstack.
pub unsafe fn pnewuserdata(
    state: *mut ffi::lua_State,
    size: usize,
    msgh: c_int,
) -> Result<*mut c_void, c_int> {
    unsafe extern "C" fn newuserdata(state: *mut ffi::lua_State) -> c_int {
        let size = ffi::lua_touserdata(state, -1) as usize;
        ffi::lua_newuserdata(state, size);
        1
    }

    ffi::lua_pushcfunction(state, newuserdata);
    ffi::lua_pushlightuserdata(state, size as *mut c_void);

    let ret = ffi::lua_pcall(state, 1, 1, msgh);
    if ret == ffi::LUA_OK {
        Ok(ffi::lua_touserdata(state, -1))
    } else {
        Err(ret)
    }
}

// Protected version of lua_pushcclosure, uses 2 extra stack spaces, does not call checkstack.
pub unsafe fn ppushcclosure(
    state: *mut ffi::lua_State,
    function: ffi::lua_CFunction,
    n: c_int,
    msgh: c_int,
) -> Result<(), c_int> {
    unsafe extern "C" fn pushcclosure(state: *mut ffi::lua_State) -> c_int {
        let function: ffi::lua_CFunction = mem::transmute(ffi::lua_touserdata(state, -2));
        let n = ffi::lua_touserdata(state, -1) as c_int;
        ffi::lua_pop(state, 2);
        ffi::lua_pushcclosure(state, function, n);
        1
    }

    if n == 0 {
        ffi::lua_pushcclosure(state, function, 0);
        Ok(())
    } else {
        ffi::lua_pushlightuserdata(state, function as *mut c_void);
        ffi::lua_pushlightuserdata(state, n as *mut c_void);

        let ret = ffi::lua_pcall(state, n.checked_add(2).unwrap(), 1, msgh);
        if ret == ffi::LUA_OK {
            Ok(())
        } else {
            Err(ret)
        }
    }
}

pub unsafe fn ppushlstring(
    state: *mut ffi::lua_State,
    s: *const c_char,
    len: usize,
    msgh: c_int,
) -> Result<*const c_char, c_int> {
    unsafe extern "C" fn pushlstring(state: *mut ffi::lua_State) -> c_int {
        let s = ffi::lua_touserdata(state, -2) as *const c_char;
        let len = ffi::lua_touserdata(state, -1) as usize;
        ffi::lua_pushlstring(state, s, len);
        1
    }

    ffi::lua_pushlightuserdata(state, s as *mut c_void);
    ffi::lua_pushlightuserdata(state, len as *mut c_void);

    let ret = ffi::lua_pcall(state, 2, 1, msgh);
    if ret == ffi::LUA_OK {
        // ffi::lua_tostring does not cause memory errors if the value is already a string
        Ok(ffi::lua_tostring(state, -1))
    } else {
        Err(ret)
    }
}

pub unsafe fn prawset(state: *mut ffi::lua_State, index: c_int, msgh: c_int) -> Result<(), c_int> {
    unsafe extern "C" fn rawset(state: *mut ffi::lua_State) -> c_int {
        ffi::lua_rawset(state, -3);
        0
    }

    let table_index = ffi::lua_absindex(state, index);

    ffi::lua_pushcfunction(state, rawset);
    ffi::lua_pushvalue(state, table_index);
    ffi::lua_pushvalue(state, -4);
    ffi::lua_pushvalue(state, -4);
    ffi::lua_remove(state, -5);
    ffi::lua_remove(state, -5);

    let ret = ffi::lua_pcall(state, 3, 0, msgh);
    if ret == ffi::LUA_OK {
        Ok(())
    } else {
        Err(ret)
    }
}

pub unsafe fn ptolstring(
    state: *mut ffi::lua_State,
    index: c_int,
    len: *mut usize,
    msgh: c_int,
) -> Result<*const c_char, c_int> {
    unsafe extern "C" fn tolstring(state: *mut ffi::lua_State) -> c_int {
        let len = ffi::lua_touserdata(state, -2) as *mut usize;
        ffi::lua_tolstring(state, -1, len);
        1
    }

    let index = ffi::lua_absindex(state, index);

    ffi::lua_pushcfunction(state, tolstring);
    ffi::lua_pushlightuserdata(state, len as *mut c_void);
    ffi::lua_pushvalue(state, index);

    let ret = ffi::lua_pcall(state, 2, 1, msgh);
    if ret == ffi::LUA_OK {
        ffi::lua_replace(state, index);
        // ffi::lua_tostring does not cause memory errors if the value is already a string
        Ok(ffi::lua_tostring(state, index))
    } else {
        Err(ret)
    }
}

pub unsafe fn ptostring(
    state: *mut ffi::lua_State,
    index: c_int,
    msgh: c_int,
) -> Result<*const c_char, c_int> {
    unsafe extern "C" fn tostring(state: *mut ffi::lua_State) -> c_int {
        ffi::lua_tolstring(state, -1, ptr::null_mut());
        1
    }

    let index = ffi::lua_absindex(state, index);

    ffi::lua_pushcfunction(state, tostring);
    ffi::lua_pushvalue(state, index);

    let ret = ffi::lua_pcall(state, 1, 1, msgh);
    if ret == ffi::LUA_OK {
        ffi::lua_replace(state, index);
        // ffi::lua_tostring does not cause memory errors if the value is already a string
        Ok(ffi::lua_tostring(state, index))
    } else {
        Err(ret)
    }
}
