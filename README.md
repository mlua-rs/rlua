# rlua -- High level bindings between Rust and Lua

*rlua is now deprecated in favour of mlua: see below for migration information*

`rlua` is now a thin transitional wrapper around
[`mlua`](https://github.com/mlua-rs/mlua); it is recommended to use mlua
directly for new projects and to migrate to it when convenient.  `mlua` was a
fork of `rlua` which has recently seen more development activity and new
features.

## Migration

`rlua` 0.20 includes some utilities to help transition to `mlua`, but is otherwise
just re-exporting `mlua` directly.

The main changes are:

* In `mlua`, `Lua::context()` is no longer necessary.  The methods previously on
  `Context` can now be called directly on the `Lua` object.  `rlua` 0.20 includes
  an `RluaCompat` extension trait which adds a `context()` method which can be used
  to avoid having to update code all at once.

* The `ToLua` trait has been renamed to `IntoLua`, and its conversion method `to_lua`
  is now `into_lua`.  `rlua` 0.20 includes `ToLua` as an alias for `IntoLua` and an
  extension `ToLuaCompat` which adds a `to_lua` method as a temporary convenience.

A few other changes which should be less disruptive:

* `mlua` has different defaults and options for blocking loading C libraries or
  compiled modules from Lua code or catching Rust panics.  Check the `Lua::new_with`
  and unsafe variants for the new options.
