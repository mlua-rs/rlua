## [0.15.0]
- Implement MultiValue conversion up to 16
- Small fix to prevent leaking on errors in metatable creation
- API incompatible change: New API for non-'static UserData!  Scoped UserData is
  now split into `create_static_userdata` and `create_nonstatic_userdata`
  because there are certain limitations on `create_nonstatic_userdata` that mean
  that nonstatic is not always what you want.
- Added pkg-config feature that can be used if builtin-lua is disabled to use
  pkg-config to find lua5.3 externally (thanks @acrisci!).
- API incompatible change: Add conversions for i128 and u128 and change the
  behavior of numeric conversions to not implicitly `as` cast.  Numeric
  conversions now use `num_traits::cast` internally and error when the cast
  function fails.  This errors on out of range but *not* loss of precision, so
  casting 1.1f64 to i32 will succeed, but casting (1i64 << 32) to i32 will not.
  When casting *to* lua, integers that are out of range of the lua_Integer type
  are instead converted to lua_Number.
- Allow arbitrary &[u8]-like data in `Lua::create_string`.  This uses
  `AsRef<[u8]>` so you can use &str and &String, but you can also now use
  `&[u8]`, which enables you to create non-utf8 Lua strings.

## [0.14.2]
- Another soundness fix for `Lua::scope` that is related to the last soundness
  fix, forbidding capturing 'lua arguments inside callbacks.  This, like the
  previous fix, is a breaking change, but anything that it breaks was *probably*
  unsound.

## [0.14.1]
- Update to require failure 0.1.2 and fix deprecation warnings.
- Update embedded Lua to 5.3.5
- Important soundness fix for `Lua::scope`, no longer allow callbacks created
  from `Scope` to leak their parameters.

## [0.14.0]
- Lots of performance improvements, including one major change: Lua handles no
  longer use the registry, they now instead are stored in an auxillary thread
  stack created solely for the purpose of holding ref values.  This may seem
  extremely weird, but it is vastly faster than using the registry or raw tables
  for this purpose.  The first attempt here was to use the same stack and to do
  a lot of complex bookkeeping to manage the references, and this DOES work, but
  it comes with a lot of complexity and downsides along with it.  The second
  approach of using an auxillary thread turned out to be equally fast and with
  no real downsides over the previous approach.  With all the performance
  improvements together, you can expect (VERY rough estimate) somewhere on the
  order of 30%-60% CPU time reduction in the cost of bindings, depending on
  usage patterns.
- Addition of some simple criterion.rs based benchmarking.  This is the first
  `rlua` release to focus on performance, but performance will hopefully remain
  a focus going forward.
- API incompatible change: `Lua` is no longer `RefUnwindSafe` and associated
  handle values are no longer `UnwindSafe` or `RefUnwindSafe`.  They should not
  have been marked as such before, because they are *extremely* internally
  mutable, so this can be considered a bugfix.  All `rlua` types should actually
  be perfectly panic safe as far as *internal* invariants are concerned, but
  (afaict) they should not be marked as `RefUnwindSafe` due to internal
  mutability and thus potentially breaking *user* invariants.
- Upgrade to require `cc` 1.0.
- Several Lua stack checking bugs have been fixed that could have lead to
  unsafety in release mode.

## [0.13.0]
- Small API incompatible change which fixes unsafety: Scope and scope created
  handle lifetimes have been changed to disallow them from escaping the scope
  callback.  Otherwise, this could lead to dangling registry handles, which can
  be used to cause UB.  This is the only API change for 0.13.
- Small fixes for potential panics / longjmps around the embedded traceback
  function.
- Temporary fix for #71 that works on stable rust without dirty tricks, while
  waiting for the larger fix for rust #48251 to make its way to stable.

## [0.12.2]
- Some minor documentation fixes.
- Fix for some rare panics which might result in an abort from panicking across
  a C API boundary.

## [0.12.1]
- Fix a stupid bug where `AnyUserData::set_user_value` /
  `AnyUserData::get_user_value` could panic if the `ToLua` / `FromLua` type
  conversion failed.
- Add `UserDataMethods::add_function_mut` and
  `UserDataMethods::add_meta_function_mut` for symmetry.
- Add some more documentation for changes in 0.12, and fix some minor problems.

## [0.12.0]
- Changed how userdata values are garbage collected, both to fix potential
  panics and to simplify it.  Now, when userdata is garbage collected, it will
  be given a special "destructed userdata" metatable, and all interactions with
  it will error with `CallbackDestructed`.  From the rust side, an expired
  userdata `AnyUserData` will not appear to be any rust type.
- Changed the `RegistryKey` API to be more useful and general.  Now, it is not
  100% necessary to manually remove `RegistryKey`s in order to clean up the
  registry, instead you can periodically call `Lua::expire_registry_values` to
  remove registry values with `RegistryKey`s that have all been dropped.  Also,
  it is no longer a panic to use a `RegistryKey` from a mismatched Lua instance,
  it is simply an error.
- Lua is now `Send`, and all userdata / callback functions have a Send
  requirement.  This is a potentially annoying breaking change, but there is a
  new way to pass !Send types to Lua in a limited way.
- HUGE change, there is now a `Lua::scope` method, which allows passing
  non-'static functions to Lua in a controlled way.  It also allows passing
  !Send functions and !Send userdata to Lua, with the same limitations.  In
  order to make this safe, the scope method behaves similarly to the `crossbeam`
  crate's `crossbeam::scope` method, which ensures that types created within the
  scope are destructed at the end of the scope.  When using callbacks / userdata
  created within the scope, the callbacks / userdata are guaranteed to be
  destructed at the end of the scope, and inside Lua references to them are in
  an invalidated "destructed" state.  This destructed state was already possible
  to observe through `__gc` methods, so it doesn't introduce anything new, but
  it has been fixed so that it cannot cause panics, and has a specific error
  type.
- Correctly error on passing too many arguments to an `rlua::Function`, and
  correctly error when returning too many results from a callback.  Previously,
  this was a panic.
- `Lua::create_function` is now split into `Lua::create_function` and
  `Lua::create_function_mut`, where the first takes a Fn and the second takes a
  FnMut.  This allows for recursion into rust functions if the function is not
  FnMut.  There is a similar change for `UserDataMethods`, where the mut
  variants of the functions now take `FnMut`, and the non-mut variants take
  `Fn`.  There is not a way to make a non-mut `UserDataMethods` method with a
  FnMut function.

## [0.11.0]
- `rlua::Error` now implements `failure::Fail` and not `std::error::Error`, and
  external errors now require `failure::Fail`.  This is the only API
  incompatible change for 0.11, and my hope is that it is relatively minor.
  There are no additional bounds on external errors, since there is a blanket
  impl for `T: std::error::Error + Send + Sync` of `failure::Fail`, but
  `rlua::Error` no longer implements `std::error::Error` and there is an
  additional dependency, and that is more likely to cause breakage.
- protect a call to `luaL_ref` when creating new userdata types.
- Some documentation improvements for `Error`, `Lua::create_function`, and
  `MetaMethod`, and a rustdoc warning fix (thanks @jonas-schievink!)
- Expose the `RegistryKey` type in the API properly, which makes the API around
  it vastly easier to use!  Also fixes a safety hole around using the
  `RegistryKey` API with the wrong `Lua` instance.
- Add an API for "user values", which are arbitrary Lua values that can be
  attached to userdata.

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
