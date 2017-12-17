# rlua -- High level bindings between Rust and Lua

[![Build Status](https://travis-ci.org/chucklefish/rlua.svg?branch=master)](https://travis-ci.org/chucklefish/rlua)

[API Documentation](https://docs.rs/rlua)

[Examples](examples/examples.rs)

This library is a high level interface between Rust and Lua.  Its major goal is
to expose as easy to use, practical, and flexible of an API between Rust and Lua
as possible, while also being completely safe.

There are other high level Lua bindings systems for rust, and this crate is an
exploration of a different part of the design space.  The other high level
interface to Lua that I am aware of right now is
[hlua](https://github.com/tomaka/hlua/) which you should definitely check out
and use if it suits your needs.  This crate has the following differences with
hlua:

  * Handles to Lua values use the Lua registry, not the stack
  * Handles to Lua values are all internally mutable
  * Handles to Lua values have non-mutable borrows to the main Lua object, so
    there can be multiple handles or long lived handles
  * Targets Lua 5.3

The key difference here is that rlua handles rust-side references to Lua values
in a fundamentally different way than hlua, more similar to other Lua bindings
systems like [Selene](https://github.com/jeremyong/Selene) for C++.  Values like
`rlua::Table` and `rlua::Function` that hold onto Lua values in the Rust stack,
instead of pointing at values in the Lua stack, are placed into the registry
with luaL_ref.  In this way, it is possible to have an arbitrary number of
handles to internal Lua values at any time, created and destroyed in arbitrary
order.  This approach IS slightly slower than the approach that hlua takes of
only manipulating the Lua stack, but this, combined with internal mutability,
allows for a much more flexible API.

There are currently a few notable missing pieces of this API:

  * Complete panic / abort safety.  This is a near term goal, but currently
    there are ways to cause panics / aborts with the API and with lua scripts.
  * Security limits on Lua code such as total instruction limits and control
    over which potentially dangerous libraries (e.g. io) are available to
    scripts.
  * Lua profiling support
  * "Context" or "Sandboxing" support.  There should be the ability to set the
    `_ENV` upvalue of a loaded chunk to a table other than `_G`, so that you can
    have different environments for different loaded chunks.
  * Benchmarks, and quantifying performance differences with what you would
    might write in C.

Additionally, there are ways I would like to change this API, once support lands
in rustc.  For example:

  * Currently, variadics are handled entirely with tuples and traits implemented
    by macro for tuples up to size 12, it would be great if this was replaced
    with real variadic generics when this is available in rust.

It is also worth it to list some non-goals for the project:

  * Be a perfect zero cost wrapper over the Lua C API
  * Allow the user to do absolutely everything that the Lua C API might allow

## API stability

This library is very much Work In Progress, so there is a some API churn.
Currently, it follows a pre-1.0 semver, so all API changes should be accompanied
by 0.x version bumps.

## Safety and panics

The goal of this library is complete safety, it should not be possible to cause
undefined behavior whatsoever with the API, even in edge cases.  There is,
however, QUITE a lot of unsafe code in this crate, and I would call the current
safety level of the crate "Work In Progress".  Still, UB is considered the most
serious kind of bug, so if you find the ability to cause UB with this API *at
all*, please file a bug report.

There are, however, currently a few known ways to cause *panics* and even
*aborts* with this API.  There is a near term goal to completely eliminate all
ways to cause panics / aborts from scripts, so many of these can be considered
bugs, but since they're known only file a bug repor if you notice any behavior
that does not match what's described here.

Panic / abort considerations when using this API:

  * The API should be panic safe currently, whenever a panic is generated the
    Lua stack is cleared and the `Lua` instance should continue to be usable.
  * Panic unwinds in Rust callbacks should currently be handled correctly, the
    unwind is caught and carried across the Lua API boundary, and Lua code
    cannot catch rust panics.  This is done by overriding the normal Lua 'pcall'
    and 'xpcall' with custom versions that cannot catch rust panics being piped
    through the normal Lua error system.
  * There are a few panics marked "internal error" that should be impossible to
    trigger.  If you encounter one of these this is a bug.
  * When the internal version of Lua is built using the `gcc` crate (the
    default), `LUA_USE_APICHECK` is enabled.  Any abort caused by this internal
    Lua API checking should be considered a bug, particularly because without
    `LUA_USE_APICHECK` it would generally be unsafe.
  * The library internally calls lua_checkstack to ensure that there is
    sufficient stack space, and if the stack cannot be sufficiently grown this
    is a panic.  There should not be a way to cause this using this API, and if
    you encounter this, it is a bug.
  * Previous to version 0.10, `rlua` had a complicated system to guard against
    LUA_ERRGCMM, and this system could cause aborts.  This is no longer the case
    as of version 0.10, `rlua` now attempts to handle all errors that the Lua C
    API can generate, including functions that can cause memory errors (any
    function marked as 'v', 'e', or 'm' in the Lua C API docs).  This is,
    however, extremely complicated and difficult to test, so if there is any
    indication when using this API that a Lua error (longjmp) is being triggered
    without being turned into an `rlua::Error`, *please please report this as a
    bug*.
  * The internal Lua allocator is set to use `realloc` from `libc`, but it is
    wrapped in such a way that OOM errors are guaranteed to abort.  This is not
    such a big deal, as this matches the behavior of rust itself.  This allows
    the internals of `rlua` to, in certain cases, call 'm' Lua C API functions
    with the garbage collector disabled and know that these cannot error.
  * There are currently no recursion limits on callbacks.  This could cause one
    of two problems, either the API will run out of stack space and cause a
    panic in Rust, or more likely it will cause an internal `LUA_USE_APICHECK`
    abort, from exceeding LUAI_MAXCCALLS.  This may be a source of unsafety if
    `LUA_USE_APICHECK` is disabled, and is considered a bug.
  * There are currently no checks on argument sizes, and I think you may be able
    to cause an abort by providing a large enough `rlua::Variadic`.  I believe
    this would be unsafe without `LUA_USE_APICHECK` and should be considered a
    bug.
