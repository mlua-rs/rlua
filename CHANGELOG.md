## [0.10.2]
- Registry API for storing values inside the `Lua` instance, either by string or
  by automatically generated keys.
- Important memory safety fix for `luaL_ref`.

## [0.10.1]
- Documentation spelling fix

## [0.10.0]
- Handle all 'm' functions in the Lua C API correctly, remove LUA_ERRGCMM hacks.
- Lots and lots of internal changes to support handling all 'm' errors
- Change the API in a lot of places due to functions that can trigger the gc now
  potentially causing Error::GarbageCollectorError errors.

## [0.9.7]
- Add unsafe function to load the debug Lua module (thanks @Timidger!)
- Fix setmetatable wrapper with nil metatable (thanks again to @Timidger!)

## [0.9.6]
- Fix an annoying bug that made external errors appear to have no further cause
  errors in the cause chain.

## [0.9.5]
- Fix incorrect `xpcall` behavior
- Change FromLua / ToLua impls for HashMap to be generic over the hasher.  This
  may be technically a backwards incompatible change, but this would be really
  unusual though, and I don't think it deserves an API bump.

## [0.9.4]
- Fix quadratic behavior in ``Function::bind``
- `lua_checkstack` fixes, particularly fixing a crash bug due to luaL_ref using
  a single extra stack space.

## [0.9.3]
- Soundness fix for recursive function calls, now causes a panic.
  This is temporary while I work on a more major update that
  prevents panics / aborts from scripts.

## [0.9.2]
- Bugfix, don't load the "base" library into the "base" global variable
  @jonas-schievink
- Additional documentation work, a link fix for `Variadic` docs, new crate
  documentation @jonas-schievink
- Metatable access on `Table`
- `gcc` crate warning fix for 0.3.52 and up
- Bugfix for `Table::raw_get`, now actually calls raw_get and is sound.

## [0.9.1]
- Add travis badge

## [0.9.0]
- Huge API change, removed the `Lua` prefix on all types, changes to callback
  signature that remove the need for manual wrapping and unwrapping in most
  cases.
- Tons of soundness bugfixes, very few soundness problems remain.
- Tons of documentation and bugifx work @jonas-schievink

## [0.8.0]
- Major API change, out of stack space is no longer an Err, you should not be
  able to run out of stack space by using this API, except through bugs.
- Simplification of error types

## [0.7.0]
- API change to remove dependency on `error_chain`, major changes to error
  handling strategy to allow Lua to catch and rethrow rust errors sanely.
