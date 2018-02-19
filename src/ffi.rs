#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(unused)]

use std::ptr;
use std::mem;
use std::os::raw::{c_char, c_double, c_int, c_longlong, c_void};

pub type lua_Integer = c_longlong;
pub type lua_Number = c_double;

pub enum lua_State {}
pub type lua_Alloc =
    unsafe extern "C" fn(ud: *mut c_void, ptr: *mut c_void, osize: usize, nsize: usize)
        -> *mut c_void;
pub type lua_KContext = *mut c_void;
pub type lua_KFunction =
    unsafe extern "C" fn(state: *mut lua_State, status: c_int, ctx: lua_KContext) -> c_int;
pub type lua_CFunction = unsafe extern "C" fn(state: *mut lua_State) -> c_int;

pub const LUA_OK: c_int = 0;
pub const LUA_YIELD: c_int = 1;
pub const LUA_ERRRUN: c_int = 2;
pub const LUA_ERRSYNTAX: c_int = 3;
pub const LUA_ERRMEM: c_int = 4;
pub const LUA_ERRGCMM: c_int = 5;
pub const LUA_ERRERR: c_int = 6;

pub const LUA_NOREF: c_int = -2;
pub const LUA_REFNIL: c_int = -1;

pub const LUA_MULTRET: c_int = -1;
pub const LUAI_MAXSTACK: c_int = 1_000_000;
pub const LUA_REGISTRYINDEX: c_int = -LUAI_MAXSTACK - 1000;
pub const LUA_RIDX_MAINTHREAD: lua_Integer = 1;
pub const LUA_RIDX_GLOBALS: lua_Integer = 2;

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
pub const LUA_GCISRUNNING: c_int = 9;

#[link(name = "lua5.3")]
extern "C" {
    // "Safe" Lua functions
    pub fn luaL_unref(state: *mut lua_State, table: c_int, lref: c_int);
    pub fn lua_atpanic(state: *mut lua_State, panic: lua_CFunction) -> lua_CFunction;
    pub fn lua_checkstack(state: *mut lua_State, n: c_int) -> c_int;
    pub fn lua_close(state: *mut lua_State);
    pub fn lua_copy(state: *mut lua_State, from: c_int, to: c_int);
    pub fn lua_getmetatable(state: *mut lua_State, index: c_int) -> c_int;
    pub fn lua_gettop(state: *const lua_State) -> c_int;
    pub fn lua_getupvalue(state: *mut lua_State, funcindex: c_int, n: c_int) -> *const c_char;
    pub fn lua_getuservalue(state: *mut lua_State, index: c_int) -> c_int;
    pub fn lua_isinteger(state: *mut lua_State, index: c_int) -> c_int;
    pub fn lua_newstate(alloc: lua_Alloc, ud: *mut c_void) -> *mut lua_State;
    pub fn lua_pushboolean(state: *mut lua_State, b: c_int);
    pub fn lua_pushinteger(state: *mut lua_State, n: lua_Integer);
    pub fn lua_pushlightuserdata(state: *mut lua_State, data: *mut c_void);
    pub fn lua_pushnil(state: *mut lua_State);
    pub fn lua_pushnumber(state: *mut lua_State, n: lua_Number);
    pub fn lua_pushvalue(state: *mut lua_State, index: c_int);
    pub fn lua_rawequal(state: *mut lua_State, index1: c_int, index2: c_int) -> c_int;
    pub fn lua_rawget(state: *mut lua_State, index: c_int) -> c_int;
    pub fn lua_rawgeti(state: *mut lua_State, index: c_int, n: lua_Integer) -> c_int;
    pub fn lua_rawlen(state: *mut lua_State, index: c_int) -> usize;
    pub fn lua_resume(state: *mut lua_State, from: *mut lua_State, nargs: c_int) -> c_int;
    pub fn lua_rotate(state: *mut lua_State, index: c_int, n: c_int);
    pub fn lua_setmetatable(state: *mut lua_State, index: c_int);
    pub fn lua_settop(state: *mut lua_State, n: c_int);
    pub fn lua_setupvalue(state: *mut lua_State, funcindex: c_int, n: c_int) -> *const c_char;
    pub fn lua_setuservalue(state: *mut lua_State, index: c_int);
    pub fn lua_status(state: *mut lua_State) -> c_int;
    pub fn lua_toboolean(state: *mut lua_State, index: c_int) -> c_int;
    pub fn lua_tointegerx(state: *mut lua_State, index: c_int, isnum: *mut c_int) -> lua_Integer;
    pub fn lua_tolstring(state: *mut lua_State, index: c_int, len: *mut usize) -> *const c_char;
    pub fn lua_tonumberx(state: *mut lua_State, index: c_int, isnum: *mut c_int) -> lua_Number;
    pub fn lua_tothread(state: *mut lua_State, index: c_int) -> *mut lua_State;
    pub fn lua_touserdata(state: *mut lua_State, index: c_int) -> *mut c_void;
    pub fn lua_type(state: *mut lua_State, index: c_int) -> c_int;

    // Unsafe Lua functions
    pub fn luaL_checkstack(state: *mut lua_State, size: c_int, msg: *const c_char);
    pub fn luaL_ref(state: *mut lua_State, table: c_int) -> c_int;
    pub fn luaL_requiref(
        state: *mut lua_State,
        modname: *const c_char,
        openf: lua_CFunction,
        glb: c_int,
    );
    pub fn luaL_traceback(
        push_state: *mut lua_State,
        state: *mut lua_State,
        msg: *const c_char,
        level: c_int,
    );
    pub fn lua_error(state: *mut lua_State) -> !;
    pub fn lua_gc(state: *mut lua_State, what: c_int, data: c_int) -> c_int;
    pub fn lua_gettable(state: *mut lua_State, index: c_int) -> c_int;
    pub fn lua_newuserdata(state: *mut lua_State, size: usize) -> *mut c_void;
    pub fn lua_pushcclosure(state: *mut lua_State, function: lua_CFunction, n: c_int);
    pub fn lua_pushlstring(state: *mut lua_State, s: *const c_char, len: usize) -> *const c_char;
    pub fn lua_pushstring(state: *mut lua_State, s: *const c_char) -> *const c_char;
    pub fn lua_rawset(state: *mut lua_State, index: c_int);
    pub fn lua_settable(state: *mut lua_State, index: c_int);

    pub fn luaopen_base(state: *mut lua_State) -> c_int;
    pub fn luaopen_coroutine(state: *mut lua_State) -> c_int;
    pub fn luaopen_debug(state: *mut lua_State) -> c_int;
    pub fn luaopen_io(state: *mut lua_State) -> c_int;
    pub fn luaopen_math(state: *mut lua_State) -> c_int;
    pub fn luaopen_os(state: *mut lua_State) -> c_int;
    pub fn luaopen_package(state: *mut lua_State) -> c_int;
    pub fn luaopen_string(state: *mut lua_State) -> c_int;
    pub fn luaopen_table(state: *mut lua_State) -> c_int;
    pub fn luaopen_utf8(state: *mut lua_State) -> c_int;
}

#[link(name = "rlua_cbits")]
extern "C" {
    // "Safe" Lua macros
    pub fn luaL_loadbuffer(
        state: *mut lua_State,
        buf: *const c_char,
        size: usize,
        name: *const c_char,
    ) -> c_int;
    pub fn lua_insert(state: *mut lua_State, index: c_int);
    pub fn lua_isnil(state: *mut lua_State, index: c_int) -> c_int;
    pub fn lua_pcall(state: *mut lua_State, nargs: c_int, nresults: c_int, msgh: c_int) -> c_int;
    pub fn lua_pop(state: *mut lua_State, n: c_int);
    pub fn lua_remove(state: *mut lua_State, index: c_int);
    pub fn lua_replace(state: *mut lua_State, index: c_int);
    pub fn lua_tointeger(state: *mut lua_State, index: c_int) -> lua_Integer;
    pub fn lua_tonumber(state: *mut lua_State, index: c_int) -> lua_Number;
    pub fn lua_pushcfunction(state: *mut lua_State, function: lua_CFunction);

    // Unsafe Lua macros
    pub fn lua_call(state: *mut lua_State, nargs: c_int, nresults: c_int);
    pub fn lua_newtable(state: *mut lua_State);
    pub fn lua_tostring(state: *mut lua_State, index: c_int) -> *const c_char;

    // Custom functions that are pcall wrappers around other Lua functions / macros.  All return a
    // success / error code, and on error will leave the error on the top of the stack.

    // Uses 1 stack space.
    pub fn plua_newtable(state: *mut lua_State) -> c_int;
    // Uses 2 stack spaces.
    pub fn pluaL_len(state: *mut lua_State, index: c_int, len: *mut lua_Integer) -> c_int;
    // Uses 3 stack spaces.
    pub fn plua_geti(state: *mut lua_State, index: c_int, i: lua_Integer) -> c_int;
    // Uses 2 stack spaces.
    pub fn plua_gettable(state: *mut lua_State, index: c_int) -> c_int;
    // Uses 1 stack space.
    pub fn plua_newthread(state: *mut lua_State, thread: *mut *mut lua_State) -> c_int;
    // Uses 2 stack spaces.
    pub fn plua_newuserdata(state: *mut lua_State, size: usize, ud: *mut *mut c_void) -> c_int;
    // Uses 2 stack spaces.
    pub fn plua_next(state: *mut lua_State, index: c_int, res: *mut c_int) -> c_int;
    // Uses 2 stack spaces.
    pub fn plua_pushcclosure(state: *mut lua_State, function: lua_CFunction, n: c_int) -> c_int;
    // Uses 3 stack spaces.
    pub fn plua_pushlstring(state: *mut lua_State, s: *const c_char, len: usize) -> c_int;
    // Uses 2 stack spaces.
    pub fn plua_pushstring(state: *mut lua_State, s: *const c_char) -> c_int;
    // Uses 4 stack spaces.
    pub fn plua_rawset(state: *mut lua_State, index: c_int) -> c_int;
    // Uses 4 stack spaces.
    pub fn plua_settable(state: *mut lua_State, index: c_int) -> c_int;
    // Uses 3 stack spaces.
    pub fn plua_tostring(state: *mut lua_State, index: c_int, s: *mut *const c_char) -> c_int;

    // Pushes a closure with a different protocol which allows errors without calling longjmp.  The
    // return codes can either be positive for a set number of return values, LUA_MULTRET, or one of
    // the RCALL_XXX constants.  Always uses the rust function pointer as the first upvalue, so the
    // maximum upvalues supported is 254, and given upvalues start at 2.  Uses one extra stack
    // space.
    pub fn plua_pushrclosure(state: *mut lua_State, function: RustCallback, n: c_int) -> c_int;
}

pub type RustCallback = unsafe extern "C" fn(state: *mut lua_State) -> c_int;

// Out of stack space in callback
pub const RCALL_STACK_ERR: c_int = -2;
// Throw the error at the top of the stack
pub const RCALL_ERR: c_int = -3;

// Inlined lua.h macros

pub fn lua_upvalueindex(i: c_int) -> c_int {
    LUA_REGISTRYINDEX - i
}

pub unsafe fn lua_getextraspace(state: *mut lua_State) -> *mut c_void {
    (state as *mut c_void).offset(-(mem::size_of::<*mut c_void>() as isize))
}
