#include "lua.h"
#include "lauxlib.h"

static int s_newtable(lua_State* state) {
    lua_newtable(state);
    return 1;
}

int plua_newtable(lua_State* state) {
    lua_pushcfunction(state, s_newtable);
    return lua_pcall(state, 0, 1, 0);
}

static int s_len(lua_State* state) {
    lua_pushinteger(state, luaL_len(state, -1));
    return 1;
}

int pluaL_len(lua_State* state, int index, lua_Integer* len) {
    index = lua_absindex(state, index);
    lua_pushcfunction(state, s_len);
    lua_pushvalue(state, index);
    int r = lua_pcall(state, 1, 1, 0);
    if (r == LUA_OK) {
        *len = lua_tointeger(state, -1);
        lua_pop(state, 1);
    }
    return r;
}

static int s_geti(lua_State* state) {
    lua_gettable(state, -2);
    return 1;
}

int plua_geti(lua_State* state, int index, lua_Integer i) {
    index = lua_absindex(state, index);
    lua_pushcfunction(state, s_geti);
    lua_pushvalue(state, index);
    lua_pushinteger(state, i);
    return lua_pcall(state, 2, 1, 0);
}

static int s_gettable(lua_State* state) {
    lua_gettable(state, -2);
    return 1;
}

int plua_gettable(lua_State* state, int index) {
    index = lua_absindex(state, index);
    lua_pushcfunction(state, s_gettable);
    lua_pushvalue(state, index);
    lua_rotate(state, -3, -1);
    return lua_pcall(state, 2, 1, 0);
}

static int s_newthread(lua_State* state) {
    lua_newthread(state);
    return 1;
}

int plua_newthread(lua_State* state, lua_State** thread) {
    lua_pushcfunction(state, s_newthread);
    int r = lua_pcall(state, 0, 1, 0);
    if (r == LUA_OK) {
        *thread = lua_tothread(state, -1);
    }
    return r;
}

static int s_newuserdata(lua_State* state) {
    size_t size = lua_tointeger(state, -1);
    lua_pop(state, 1);
    lua_newuserdata(state, size);
    return 1;
}

int plua_newuserdata(lua_State* state, size_t size, void** ud) {
    lua_pushcfunction(state, s_newuserdata);
    lua_pushinteger(state, size);
    int r = lua_pcall(state, 1, 1, 0);
    if (r == LUA_OK) {
        *ud = lua_touserdata(state, -1);
    }
    return r;
}

static int s_next(lua_State* state) {
    if (lua_next(state, -2) == 0) {
        return 0;
    } else {
        return 2;
    }
}

int plua_next(lua_State* state, int index, int* res) {
    int top = lua_gettop(state) - 1;
    index = lua_absindex(state, index);
    lua_pushcfunction(state, s_next);
    lua_pushvalue(state, index);
    lua_rotate(state, -3, -1);
    int r = lua_pcall(state, 2, LUA_MULTRET, 0);
    if (r == LUA_OK) {
        if (lua_gettop(state) - top == 2) {
            *res = 1;
        } else {
            *res = 0;
        }
    }
    return r;
}

static int s_pushcclosure(lua_State* state) {
    lua_CFunction cf = lua_tocfunction(state, -1);
    lua_pop(state, 1);
    lua_pushcclosure(state, cf, lua_gettop(state));
    return 1;
}

int plua_pushcclosure(lua_State* state, lua_CFunction function, int n) {
    lua_pushcfunction(state, s_pushcclosure);
    lua_insert(state, -(n + 1));
    lua_pushcfunction(state, function);
    return lua_pcall(state, n + 1, 1, 0);
}

static int s_pushlstring(lua_State* state) {
    char const* s = lua_touserdata(state, -2);
    size_t len = lua_tointeger(state, -1);
    lua_pop(state, 2);
    lua_pushlstring(state, s, len);
    return 1;
}

int plua_pushlstring(lua_State* state, char const* s, size_t len) {
    lua_pushcfunction(state, s_pushlstring);
    lua_pushlightuserdata(state, (void*)s);
    lua_pushinteger(state, len);
    return lua_pcall(state, 2, 1, 0);
}

static int s_pushstring(lua_State* state) {
    char const* s = lua_touserdata(state, -1);
    lua_pop(state, 1);
    lua_pushstring(state, s);
    return 1;
}

int plua_pushstring(lua_State* state, char const* s) {
    lua_pushcfunction(state, s_pushstring);
    lua_pushlightuserdata(state, (void*)s);
    return lua_pcall(state, 1, 1, 0);
}

static int s_rawset(lua_State* state) {
    lua_rawset(state, -3);
    return 0;
}

int plua_rawset(lua_State* state, int index) {
    lua_pushvalue(state, index);
    lua_insert(state, -3);
    lua_pushcfunction(state, s_rawset);
    lua_insert(state, -4);
    return lua_pcall(state, 3, 0, 0);
}

static int s_settable(lua_State* state) {
    lua_settable(state, -3);
    return 0;
}

int plua_settable(lua_State* state, int index) {
    lua_pushvalue(state, index);
    lua_insert(state, -3);
    lua_pushcfunction(state, s_settable);
    lua_insert(state, -4);
    return lua_pcall(state, 3, 0, 0);
}

static int s_tostring(lua_State* state) {
    char const** s = lua_touserdata(state, -1);
    *s = lua_tostring(state, -2);
    lua_pop(state, 1);
    return 1;
}

int plua_tostring(lua_State* state, int index, char const** s) {
    index = lua_absindex(state, index);
    lua_pushcfunction(state, s_tostring);
    lua_pushvalue(state, index);
    lua_pushlightuserdata(state, s);
    int r = lua_pcall(state, 2, 1, 0);
    if (r == LUA_OK) {
        lua_replace(state, index);
    }
    return r;
}

typedef int (*RustCallback)(lua_State*);
// Out of stack space in callback
static int const RCALL_STACK_ERR = -2;
// Throw the error at the top of the stack
static int const RCALL_ERR = -3;

static int s_call_rust(lua_State* state) {
    RustCallback callback = lua_touserdata(state, lua_upvalueindex(1));
    int ret = callback(state);

    if (ret == LUA_MULTRET) {
        return LUA_MULTRET;
    } else if (ret == RCALL_STACK_ERR) {
        return luaL_error(state, "stack overflow in rust callback");
    } else if (ret == RCALL_ERR) {
        return lua_error(state);
    } else {
        return ret;
    }
}

int plua_pushrclosure(lua_State* state, RustCallback function, int n) {
    lua_pushlightuserdata(state, function);
    lua_insert(state, -(n + 1));
    return plua_pushcclosure(state, s_call_rust, n + 1);
}
