use std::mem;
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;

#[allow(non_upper_case_globals)]
#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
pub mod bindings;

pub const LUA_OK: c_int = bindings::LUA_OK as c_int;
pub const LUA_YIELD: c_int = bindings::LUA_YIELD as c_int;
pub const LUA_ERRRUN: c_int = bindings::LUA_ERRRUN as c_int;
pub const LUA_ERRSYNTAX: c_int = bindings::LUA_ERRSYNTAX as c_int;
pub const LUA_ERRMEM: c_int = bindings::LUA_ERRMEM as c_int;
pub const LUA_ERRERR: c_int = bindings::LUA_ERRERR as c_int;

pub const LUA_NOREF: c_int = bindings::LUA_NOREF as c_int;
pub const LUA_REFNIL: c_int = bindings::LUA_REFNIL as c_int;

pub const LUA_MULTRET: c_int = bindings::LUA_MULTRET as c_int;
pub const LUAI_MAXSTACK: c_int = bindings::LUAI_MAXSTACK as c_int;
pub const LUA_REGISTRYINDEX: c_int = bindings::LUA_REGISTRYINDEX as c_int;
pub const LUA_RIDX_MAINTHREAD: bindings::lua_Integer =
    bindings::LUA_RIDX_MAINTHREAD as bindings::lua_Integer;
pub const LUA_RIDX_GLOBALS: bindings::lua_Integer =
    bindings::LUA_RIDX_GLOBALS as bindings::lua_Integer;
pub const LUA_IDSIZE: c_int = bindings::LUA_IDSIZE as c_int;
pub const LUA_MINSTACK: c_int = bindings::LUA_MINSTACK as c_int;
// Not actually defined in lua.h / luaconf.h
pub const LUA_MAX_UPVALUES: c_int = 255;

pub const LUA_TNONE: c_int = bindings::LUA_TNONE as c_int;
pub const LUA_TNIL: c_int = bindings::LUA_TNIL as c_int;
pub const LUA_TBOOLEAN: c_int = bindings::LUA_TBOOLEAN as c_int;
pub const LUA_TLIGHTUSERDATA: c_int = bindings::LUA_TLIGHTUSERDATA as c_int;
pub const LUA_TNUMBER: c_int = bindings::LUA_TNUMBER as c_int;
pub const LUA_TSTRING: c_int = bindings::LUA_TSTRING as c_int;
pub const LUA_TTABLE: c_int = bindings::LUA_TTABLE as c_int;
pub const LUA_TFUNCTION: c_int = bindings::LUA_TFUNCTION as c_int;
pub const LUA_TUSERDATA: c_int = bindings::LUA_TUSERDATA as c_int;
pub const LUA_TTHREAD: c_int = bindings::LUA_TTHREAD as c_int;

pub const LUA_GCSTOP: c_int = bindings::LUA_GCSTOP as c_int;
pub const LUA_GCRESTART: c_int = bindings::LUA_GCRESTART as c_int;
pub const LUA_GCCOLLECT: c_int = bindings::LUA_GCCOLLECT as c_int;
pub const LUA_GCCOUNT: c_int = bindings::LUA_GCCOUNT as c_int;
pub const LUA_GCCOUNTB: c_int = bindings::LUA_GCCOUNTB as c_int;
pub const LUA_GCSTEP: c_int = bindings::LUA_GCSTEP as c_int;
#[deprecated(note = "please use `LUA_GCINC` instead")]
pub const LUA_GCSETPAUSE: c_int = bindings::LUA_GCSETPAUSE as c_int;
#[deprecated(note = "please use `LUA_GCINC` instead")]
pub const LUA_GCSETSTEPMUL: c_int = bindings::LUA_GCSETSTEPMUL as c_int;
pub const LUA_GCISRUNNING: c_int = bindings::LUA_GCISRUNNING as c_int;
pub const LUA_GCGEN: c_int = bindings::LUA_GCGEN as c_int;
pub const LUA_GCINC: c_int = bindings::LUA_GCINC as c_int;

pub const LUA_MASKCALL: c_int = bindings::LUA_MASKCALL as c_int;
pub const LUA_MASKRET: c_int = bindings::LUA_MASKRET as c_int;
pub const LUA_MASKLINE: c_int = bindings::LUA_MASKLINE as c_int;
pub const LUA_MASKCOUNT: c_int = bindings::LUA_MASKCOUNT as c_int;

pub use {
    bindings::LUA_AUTHORS, bindings::LUA_COPYRIGHT, bindings::LUA_VERSION,
    bindings::LUA_VERSION_MAJOR, bindings::LUA_VERSION_MINOR, bindings::LUA_VERSION_NUM,
    bindings::LUA_VERSION_RELEASE, bindings::LUA_VERSION_RELEASE_NUM,
};

pub use {
    bindings::lua_compare, bindings::lua_getinfo, bindings::lua_getlocal, bindings::lua_getupvalue,
    bindings::lua_rawequal, bindings::lua_sethook, bindings::lua_setlocal,
    bindings::lua_setupvalue,
};

pub use {
    bindings::lua_Alloc, bindings::lua_CFunction, bindings::lua_Debug, bindings::lua_Integer,
    bindings::lua_KContext, bindings::lua_Number, bindings::lua_State, bindings::lua_Unsigned,
    bindings::lua_setcstacklimit,
};

/*
** state manipulation
*/
pub use {
    bindings::lua_atpanic, bindings::lua_close, bindings::lua_newstate, bindings::lua_newthread,
    bindings::lua_resetthread, bindings::lua_version,
};

/*
** basic stack manipulation
*/
pub use {
    bindings::lua_absindex, bindings::lua_checkstack, bindings::lua_copy, bindings::lua_gettop,
    bindings::lua_pushvalue, bindings::lua_rotate, bindings::lua_settop, bindings::lua_xmove,
};

/*
** access functions (stack -> C)
*/
pub use {
    bindings::lua_iscfunction, bindings::lua_isinteger, bindings::lua_isnumber,
    bindings::lua_isstring, bindings::lua_isuserdata, bindings::lua_rawlen,
    bindings::lua_toboolean, bindings::lua_tocfunction, bindings::lua_tointegerx,
    bindings::lua_tolstring, bindings::lua_tonumberx, bindings::lua_topointer,
    bindings::lua_tothread, bindings::lua_touserdata, bindings::lua_type, bindings::lua_typename,
};

/*
** push functions (C -> stack)
*/
pub use {
    bindings::lua_pushboolean, bindings::lua_pushcclosure, bindings::lua_pushfstring,
    bindings::lua_pushinteger, bindings::lua_pushlightuserdata, bindings::lua_pushlstring,
    bindings::lua_pushnil, bindings::lua_pushnumber, bindings::lua_pushstring,
    bindings::lua_pushthread, bindings::lua_pushvfstring,
};

/*
** get functions (Lua -> stack)
*/
pub use {
    bindings::lua_createtable, bindings::lua_getfield, bindings::lua_getglobal, bindings::lua_geti,
    bindings::lua_getiuservalue, bindings::lua_getmetatable, bindings::lua_gettable,
    bindings::lua_newuserdatauv, bindings::lua_rawget, bindings::lua_rawgeti,
    bindings::lua_rawgetp,
};

/*
** set functions (stack -> Lua)
*/
pub use {
    bindings::lua_rawset, bindings::lua_rawseti, bindings::lua_rawsetp, bindings::lua_setfield,
    bindings::lua_setglobal, bindings::lua_seti, bindings::lua_setiuservalue,
    bindings::lua_setmetatable, bindings::lua_settable,
};

/*
** 'load' and 'call' functions (load and run Lua code)
*/

pub use {bindings::lua_callk, bindings::lua_dump, bindings::lua_load, bindings::lua_pcallk};
/*
** coroutine functions
*/
pub use {
    bindings::lua_isyieldable, bindings::lua_resume, bindings::lua_status, bindings::lua_yieldk,
};

/*
** Warning-related functions
*/
pub use {bindings::lua_setwarnf, bindings::lua_warning};

/*
** garbage-collection function and options
*/
pub use bindings::lua_gc;

/*
** miscellaneous functions
*/
pub use {
    bindings::lua_concat, bindings::lua_error, bindings::lua_getallocf, bindings::lua_len,
    bindings::lua_next, bindings::lua_setallocf, bindings::lua_stringtonumber,
    bindings::lua_toclose,
};

/*
** lauxlib.h
*/
pub use {
    bindings::luaL_Buffer, bindings::luaL_Reg, bindings::luaL_Stream, bindings::luaL_addgsub,
    bindings::luaL_addlstring, bindings::luaL_addstring, bindings::luaL_addvalue,
    bindings::luaL_argerror, bindings::luaL_buffinit, bindings::luaL_buffinitsize,
    bindings::luaL_callmeta, bindings::luaL_checkany, bindings::luaL_checkinteger,
    bindings::luaL_checklstring, bindings::luaL_checknumber, bindings::luaL_checkoption,
    bindings::luaL_checkstack, bindings::luaL_checktype, bindings::luaL_checkudata,
    bindings::luaL_error, bindings::luaL_execresult, bindings::luaL_getmetafield,
    bindings::luaL_getsubtable, bindings::luaL_gsub, bindings::luaL_len,
    bindings::luaL_loadbufferx, bindings::luaL_loadfilex, bindings::luaL_loadstring,
    bindings::luaL_newmetatable, bindings::luaL_newstate, bindings::luaL_optinteger,
    bindings::luaL_optlstring, bindings::luaL_optnumber, bindings::luaL_prepbuffsize,
    bindings::luaL_pushresult, bindings::luaL_pushresultsize, bindings::luaL_ref,
    bindings::luaL_requiref, bindings::luaL_setfuncs, bindings::luaL_setmetatable,
    bindings::luaL_testudata, bindings::luaL_tolstring, bindings::luaL_traceback,
    bindings::luaL_typeerror, bindings::luaL_unref, bindings::luaL_where,
};

/*
** lualib.h
*/
pub use {
    bindings::luaopen_base, bindings::luaopen_coroutine, bindings::luaopen_debug,
    bindings::luaopen_io, bindings::luaopen_math, bindings::luaopen_os, bindings::luaopen_package,
    bindings::luaopen_string, bindings::luaopen_table, bindings::luaopen_utf8,
};

// The following are re-implementations of what are macros in the Lua C API
pub unsafe fn lua_getextraspace(state: *mut lua_State) -> *mut c_void {
    (state as *mut c_void).offset(-(mem::size_of::<*mut c_void>() as isize))
}

pub unsafe fn lua_pcall(
    state: *mut lua_State,
    nargs: c_int,
    nresults: c_int,
    msgh: c_int,
) -> c_int {
    lua_pcallk(state, nargs, nresults, msgh, 0, None)
}

pub unsafe fn lua_newuserdata(state: *mut lua_State, s: usize) -> *mut ::std::os::raw::c_void {
    lua_newuserdatauv(state, s, 1)
}

pub unsafe fn lua_getuservalue(state: *mut lua_State, idx: c_int) -> c_int {
    lua_getiuservalue(state, idx, 1)
}

pub unsafe fn lua_setuservalue(state: *mut lua_State, idx: c_int) -> c_int {
    lua_setiuservalue(state, idx, 1)
}

pub unsafe fn lua_tonumber(state: *mut lua_State, idx: c_int) -> lua_Number {
    return lua_tonumberx(state, idx, ptr::null_mut());
}
pub unsafe fn lua_tointeger(state: *mut lua_State, idx: c_int) -> lua_Number {
    return lua_tonumberx(state, idx, ptr::null_mut());
}
pub unsafe fn lua_pop(state: *mut lua_State, n: c_int) {
    lua_settop(state, -(n) - 1);
}
pub unsafe fn lua_isfunction(state: *mut lua_State, n: c_int) -> bool {
    return lua_type(state, n) == LUA_TFUNCTION as i32;
}
pub unsafe fn lua_istable(state: *mut lua_State, n: c_int) -> bool {
    return lua_type(state, n) == LUA_TTABLE as i32;
}
pub unsafe fn lua_islightuserdata(state: *mut lua_State, n: c_int) -> bool {
    return lua_type(state, n) == LUA_TLIGHTUSERDATA as i32;
}
pub unsafe fn lua_isnil(state: *mut lua_State, n: c_int) -> bool {
    return lua_type(state, n) == LUA_TNIL as i32;
}
pub unsafe fn lua_isboolean(state: *mut lua_State, n: c_int) -> bool {
    return lua_type(state, n) == LUA_TBOOLEAN as i32;
}
pub unsafe fn lua_isthread(state: *mut lua_State, n: c_int) -> bool {
    return lua_type(state, n) == LUA_TTHREAD as i32;
}
pub unsafe fn lua_isnone(state: *mut lua_State, n: c_int) -> bool {
    return lua_type(state, n) == LUA_TNONE as i32;
}
pub unsafe fn lua_isnoneornil(state: *mut lua_State, n: c_int) -> bool {
    return lua_type(state, n) == LUA_TNONE as i32;
}

pub unsafe fn lua_pushliteral(state: *mut lua_State, str: *const c_char) -> *const c_char {
    return lua_pushstring(state, str);
}

pub unsafe fn lua_pushglobaltable(state: *mut lua_State) {
    lua_rawgeti(state, LUA_REGISTRYINDEX as i32, LUA_RIDX_GLOBALS as i64);
}

pub unsafe fn lua_newtable(state: *mut lua_State) {
    lua_createtable(state, 0, 0);
}

pub unsafe fn lua_register(state: *mut lua_State, n: *const c_char, f: lua_CFunction) {
    lua_pushcfunction(state, f);
    lua_setglobal(state, n);
}

pub unsafe fn lua_pushcfunction(state: *mut lua_State, f: lua_CFunction) {
    lua_pushcclosure(state, f, 0);
}

pub unsafe fn lua_tostring(state: *mut lua_State, i: c_int) -> *const c_char {
    return lua_tolstring(state, i, ptr::null_mut());
}

pub unsafe fn lua_insert(state: *mut lua_State, idx: c_int) {
    lua_rotate(state, idx, 1);
}

pub unsafe fn lua_remove(state: *mut lua_State, idx: c_int) {
    lua_rotate(state, idx, -1);
    lua_pop(state, 1);
}

pub unsafe fn lua_replace(state: *mut lua_State, idx: c_int) {
    lua_copy(state, -1, idx);
    lua_pop(state, 1);
}

pub unsafe fn lua_upvalueindex(index: c_int) -> i32 {
    return LUA_REGISTRYINDEX - index;
}

pub unsafe fn lua_call(state: *mut lua_State, nargs: c_int, nresults: c_int) {
    lua_callk(state, nargs, nresults, 0, None)
}
