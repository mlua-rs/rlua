#include "lua.h"
#include "lauxlib.h"

void (lua_pop)(lua_State* state, int n) {
    lua_pop(state, n);
}

void (lua_newtable)(lua_State* state) {
    lua_newtable(state);
}

void (lua_pushcfunction)(lua_State* state, lua_CFunction function) {
    lua_pushcfunction(state, function);
}

lua_Number (lua_tonumber)(lua_State* state, int index) {
    return lua_tonumber(state, index);
}

lua_Integer (lua_tointeger)(lua_State* state, int index) {
    return lua_tointeger(state, index);
}

char const* (lua_tostring)(lua_State* state, int index) {
    return lua_tostring(state, index);
}

int (lua_isfunction)(lua_State* state, int index) {
    return lua_isfunction(state, index);
}

int (lua_istable)(lua_State* state, int index) {
    return lua_istable(state, index);
}

int (lua_islightuserdata)(lua_State* state, int index) {
    return lua_islightuserdata(state, index);
}

int (lua_isnil)(lua_State* state, int index) {
    return lua_isnil(state, index);
}

int (lua_isboolean)(lua_State* state, int index) {
    return lua_isboolean(state, index);
}

int (lua_isthread)(lua_State* state, int index) {
    return lua_isthread(state, index);
}

int (lua_isnone)(lua_State* state, int index) {
    return lua_isnone(state, index);
}

void (lua_insert)(lua_State* state, int index) {
    lua_insert(state, index);
}

void (lua_remove)(lua_State* state, int index) {
    lua_remove(state, index);
}

void (lua_call)(lua_State* state, int nargs, int nresults) {
    lua_call(state, nargs, nresults);
}

int (lua_pcall)(lua_State* state, int nargs, int nresults, int msgh) {
    return lua_pcall(state, nargs, nresults, msgh);
}

void (lua_replace)(lua_State* state, int index) {
    lua_replace(state, index);
}

int (luaL_loadbuffer)(lua_State* state, char const* buf, size_t size, char const* name) {
    return luaL_loadbufferx(state, buf, size, name, NULL);
}
