#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(unused)]

use libc::ptrdiff_t;
use std::mem;
use std::os::raw::{c_char, c_double, c_int, c_longlong, c_uchar, c_void};
use std::ptr;

pub type lua_Integer = ptrdiff_t;
pub type lua_Number = c_double;

pub enum lua_State {}
pub type lua_Alloc = unsafe extern "C" fn(
    ud: *mut c_void,
    ptr: *mut c_void,
    osize: usize,
    nsize: usize,
) -> *mut c_void;
pub type lua_CFunction = unsafe extern "C" fn(state: *mut lua_State) -> c_int;
pub type lua_Hook = unsafe extern "C" fn(state: *mut lua_State, ar: *mut lua_Debug);

#[repr(C)]
pub struct lua_Debug {
    pub event: c_int,
    pub name: *const c_char,
    pub namewhat: *const c_char,
    pub what: *const c_char,
    pub source: *const c_char,
    pub currentline: c_int,
    pub nups: c_int,
    pub linedefined: c_int,
    pub lastlinedefined: c_int,
    pub short_src: [c_char; LUA_IDSIZE as usize],
    i_ci: c_int,
}

pub const LUA_OK: c_int = 0;
pub const LUA_YIELD: c_int = 1;
pub const LUA_ERRRUN: c_int = 2;
pub const LUA_ERRSYNTAX: c_int = 3;
pub const LUA_ERRMEM: c_int = 4;
pub const LUA_ERRERR: c_int = 5;

pub const LUA_NOREF: c_int = -2;
pub const LUA_REFNIL: c_int = -1;

pub const LUA_MULTRET: c_int = -1;
pub const LUAI_MAXSTACK: c_int = 1_000_000;
pub const LUA_REGISTRYINDEX: c_int = -10000;
pub const LUA_ENVIRONINDEX: c_int = -10001;
pub const LUA_GLOBALSINDEX: c_int = -10002;
pub const LUA_IDSIZE: c_int = 60;
pub const LUA_MINSTACK: c_int = 20;
// Not actually defined in lua.h / luaconf.h
pub const LUA_MAX_UPVALUES: c_int = 255;

pub const LUA_TNONE: c_int = -1;
pub const LUA_TNIL: c_int = 0;
pub const LUA_TBOOLEAN: c_int = 1;
pub const LUA_TLIGHTUSERDATA: c_int = 2;
pub const LUA_TNUMBER: c_int = 3;
pub const LUA_TSTRING: c_int = 4;
pub const LUA_TTABLE: c_int = 5;
pub const LUA_TFUNCTION: c_int = 6;
pub const LUA_TUSERDATA: c_int = 7;
pub const LUA_TTHREAD: c_int = 8;

pub const LUA_GCSTOP: c_int = 0;
pub const LUA_GCRESTART: c_int = 1;
pub const LUA_GCCOLLECT: c_int = 2;
pub const LUA_GCCOUNT: c_int = 3;
pub const LUA_GCCOUNTB: c_int = 4;
pub const LUA_GCSTEP: c_int = 5;
pub const LUA_GCSETPAUSE: c_int = 6;
pub const LUA_GCSETSTEPMUL: c_int = 7;

pub const LUA_MASKCALL: c_int = 1;
pub const LUA_MASKRET: c_int = 2;
pub const LUA_MASKLINE: c_int = 4;
pub const LUA_MASKCOUNT: c_int = 8;

extern "C" {
    pub fn lua_newstate(alloc: lua_Alloc, ud: *mut c_void) -> *mut lua_State;
    pub fn lua_close(state: *mut lua_State);

    pub fn lua_getallocf(state: *mut lua_State, ud: *mut *mut c_void) -> lua_Alloc;
    pub fn lua_setallocf(state: *mut lua_State, ud: *mut c_void);

    pub fn lua_call(state: *mut lua_State, nargs: c_int, nresults: c_int);
    pub fn lua_pcall(state: *mut lua_State, nargs: c_int, nresults: c_int, errfunc: c_int)
        -> c_int;

    pub fn lua_resume(state: *mut lua_State, nargs: c_int) -> c_int;
    pub fn lua_status(state: *mut lua_State) -> c_int;

    pub fn lua_pushnil(state: *mut lua_State);
    pub fn lua_pushvalue(state: *mut lua_State, index: c_int);
    pub fn lua_remove(state: *mut lua_State, index: c_int);
    pub fn lua_insert(state: *mut lua_State, index: c_int);
    pub fn lua_replace(state: *mut lua_State, index: c_int);
    pub fn lua_pushboolean(state: *mut lua_State, b: c_int);
    pub fn lua_pushinteger(state: *mut lua_State, n: lua_Integer);
    pub fn lua_pushnumber(state: *mut lua_State, n: lua_Number);
    pub fn lua_pushlstring(state: *mut lua_State, s: *const c_char, len: usize);
    pub fn lua_pushstring(state: *mut lua_State, s: *const c_char);
    pub fn lua_pushlightuserdata(state: *mut lua_State, data: *mut c_void);
    pub fn lua_pushcclosure(state: *mut lua_State, function: lua_CFunction, n: c_int);
    pub fn lua_pushthread(state: *mut lua_State) -> c_int;

    pub fn lua_equal(state: *mut lua_State, idx1: c_int, idx2: c_int) -> c_int;
    pub fn lua_lessthan(state: *mut lua_State, idx1: c_int, idx2: c_int) -> c_int;

    pub fn lua_tointeger(state: *mut lua_State, index: c_int) -> lua_Integer;
    pub fn lua_tolstring(state: *mut lua_State, index: c_int, len: *mut usize) -> *const c_char;
    pub fn lua_toboolean(state: *mut lua_State, index: c_int) -> c_int;
    pub fn lua_tonumber(state: *mut lua_State, index: c_int) -> lua_Number;
    pub fn lua_touserdata(state: *mut lua_State, index: c_int) -> *mut c_void;
    pub fn lua_tothread(state: *mut lua_State, index: c_int) -> *mut lua_State;
    pub fn lua_topointer(state: *mut lua_State, index: c_int) -> *const c_void;

    pub fn lua_gettop(state: *const lua_State) -> c_int;
    pub fn lua_settop(state: *mut lua_State, n: c_int);
    pub fn lua_checkstack(state: *mut lua_State, sz: c_int) -> c_int;
    pub fn lua_xmove(from: *mut lua_State, to: *mut lua_State, n: c_int);

    pub fn lua_isnumber(state: *mut lua_State, index: c_int) -> c_int;
    pub fn lua_isstring(state: *mut lua_State, index: c_int) -> c_int;
    pub fn lua_iscfunction(state: *mut lua_State, index: c_int) -> c_int;
    pub fn lua_isuserdata(state: *mut lua_State, index: c_int) -> c_int;
    pub fn lua_type(state: *mut lua_State, index: c_int) -> c_int;
    pub fn lua_typename(state: *mut lua_State, tp: c_int) -> *const c_char;

    pub fn lua_gettable(state: *mut lua_State, index: c_int);
    pub fn lua_rawget(state: *mut lua_State, index: c_int);
    pub fn lua_rawgeti(state: *mut lua_State, index: c_int, n: lua_Integer);
    pub fn lua_rawseti(state: *mut lua_State, index: c_int, n: c_int);
    pub fn lua_getmetatable(state: *mut lua_State, index: c_int) -> c_int;
    pub fn lua_setfield(state: *mut lua_State, index: c_int, k: *const c_char);
    pub fn lua_getfield(state: *mut lua_State, index: c_int, k: *const c_char);

    pub fn lua_createtable(state: *mut lua_State, narr: c_int, nrec: c_int);
    pub fn lua_newuserdata(state: *mut lua_State, size: usize) -> *mut c_void;
    pub fn lua_newthread(state: *mut lua_State) -> *mut lua_State;

    pub fn lua_getupvalue(state: *mut lua_State, funcindex: c_int, n: c_int) -> *const c_char;
    pub fn lua_setupvalue(state: *mut lua_State, funcindex: c_int, n: c_int) -> *const c_char;

    pub fn lua_settable(state: *mut lua_State, index: c_int);
    pub fn lua_rawset(state: *mut lua_State, index: c_int);
    pub fn lua_setmetatable(state: *mut lua_State, index: c_int);
    pub fn lua_setfenv(state: *mut lua_State, index: c_int) -> c_int;
    pub fn lua_getfenv(state: *mut lua_State, index: c_int);

    pub fn lua_objlen(state: *mut lua_State, index: c_int) -> usize;
    pub fn lua_next(state: *mut lua_State, index: c_int) -> c_int;
    pub fn lua_rawequal(state: *mut lua_State, index1: c_int, index2: c_int) -> c_int;

    pub fn lua_error(state: *mut lua_State) -> !;
    pub fn lua_atpanic(state: *mut lua_State, panic: lua_CFunction) -> lua_CFunction;
    pub fn lua_gc(state: *mut lua_State, what: c_int, data: c_int) -> c_int;
    pub fn lua_getinfo(state: *mut lua_State, what: *const c_char, ar: *mut lua_Debug) -> c_int;

    pub fn lua_sethook(
        state: *mut lua_State,
        f: Option<lua_Hook>,
        mask: c_int,
        count: c_int,
    ) -> c_int;

    pub fn luaopen_base(state: *mut lua_State) -> c_int;
    pub fn luaopen_table(state: *mut lua_State) -> c_int;
    pub fn luaopen_io(state: *mut lua_State) -> c_int;
    pub fn luaopen_os(state: *mut lua_State) -> c_int;
    pub fn luaopen_string(state: *mut lua_State) -> c_int;
    pub fn luaopen_utf8(state: *mut lua_State) -> c_int;
    pub fn luaopen_math(state: *mut lua_State) -> c_int;
    pub fn luaopen_debug(state: *mut lua_State) -> c_int;
    pub fn luaopen_package(state: *mut lua_State) -> c_int;

    pub fn luaL_newstate() -> *mut lua_State;

    pub fn luaL_loadbuffer(
        state: *mut lua_State,
        buf: *const c_char,
        size: usize,
        name: *const c_char,
    ) -> c_int;
    pub fn luaL_ref(state: *mut lua_State, table: c_int) -> c_int;
    pub fn luaL_unref(state: *mut lua_State, table: c_int, lref: c_int);
    pub fn luaL_checkstack(state: *mut lua_State, size: c_int, msg: *const c_char);
    pub fn luaL_callmeta(state: *mut lua_State, obj: c_int, e: *const c_char) -> c_int;
    pub fn luaL_findtable(
        state: *mut lua_State,
        idx: c_int,
        fname: *const c_char,
        szhint: c_int,
    ) -> *const c_char;
}

// The following are re-implementations of what are macros in the Lua C API

pub unsafe fn lua_pop(state: *mut lua_State, n: c_int) {
    lua_settop(state, -n - 1);
}

pub unsafe fn lua_newtable(state: *mut lua_State) {
    lua_createtable(state, 0, 0);
}

pub fn lua_upvalueindex(i: c_int) -> c_int {
    LUA_GLOBALSINDEX - i
}

pub unsafe fn lua_pushcfunction(state: *mut lua_State, function: lua_CFunction) {
    lua_pushcclosure(state, function, 0);
}

pub unsafe fn lua_tostring(state: *mut lua_State, index: c_int) -> *const c_char {
    lua_tolstring(state, index, ptr::null_mut())
}

pub unsafe fn lua_isfunction(state: *mut lua_State, index: c_int) -> c_int {
    if lua_type(state, index) == LUA_TFUNCTION {
        1
    } else {
        0
    }
}

pub unsafe fn lua_istable(state: *mut lua_State, index: c_int) -> c_int {
    if lua_type(state, index) == LUA_TTABLE {
        1
    } else {
        0
    }
}

pub unsafe fn lua_islightuserdata(state: *mut lua_State, index: c_int) -> c_int {
    if lua_type(state, index) == LUA_TLIGHTUSERDATA {
        1
    } else {
        0
    }
}

pub unsafe fn lua_isnil(state: *mut lua_State, index: c_int) -> c_int {
    if lua_type(state, index) == LUA_TNIL {
        1
    } else {
        0
    }
}

pub unsafe fn lua_isboolean(state: *mut lua_State, index: c_int) -> c_int {
    if lua_type(state, index) == LUA_TBOOLEAN {
        1
    } else {
        0
    }
}

pub unsafe fn lua_isthread(state: *mut lua_State, index: c_int) -> c_int {
    if lua_type(state, index) == LUA_TTHREAD {
        1
    } else {
        0
    }
}

pub unsafe fn lua_isnone(state: *mut lua_State, index: c_int) -> c_int {
    if lua_type(state, index) == LUA_TNONE {
        1
    } else {
        0
    }
}

pub unsafe fn lua_setglobal(state: *mut lua_State, name: *const c_char) {
    lua_setfield(state, LUA_GLOBALSINDEX, name);
}
