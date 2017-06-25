# rlua -- High level bindings between Rust and Lua

[![Build Status](https://travis-ci.org/chucklefish/rlua.svg?branch=master)](https://travis-ci.org/chucklefish/rlua)

[WIP API Documentation](https://docs.rs/rlua)

This library is a WIP high level interface between Rust and Lua.  Its major
goal is to expose as easy to use, practical, and flexible of an API between
Rust and Lua as possible, while also being completely safe.

There are other high level Lua bindings systems for rust, and this crate is an
exploration of a different part of the design space.  The main high level
interface to Lua right now is [hlua](https://github.com/tomaka/hlua/) which you
should definitely check out and use if it suits your needs.  This crate has the
following differences with hlua:

  * Handles to Lua values use the Lua registry, not the stack
  * Handles to Lua values are all internally mutable
  * Handles to Lua values have non-mutable borrows to the main Lua object, so
    there can be multiple handles or long lived handles
  * Targets Lua 5.3

The key difference here is that rlua handles rust-side references to Lua values
in a fundamentally different way than hlua, more similar to other Lua bindings
systems like [Selene](https://github.com/jeremyong/Selene) for C++.  Values like
LuaTable and LuaFunction that hold onto Lua values in the Rust stack, instead of
pointing at values in the Lua stack, are placed into the registry with luaL_ref.
In this way, it is possible to have an arbitrary number of handles to internal
Lua values at any time, created and destroyed in arbitrary order.  This approach
IS slightly slower than the approach that hlua takes of only manipulating the
Lua stack, but this, combined with internal mutability, allows for a much more
flexible API.

There are currently a few notable missing pieces of this API:

  * Security limits on Lua code such as total instruction limits and recursion
    limits to prevent DOS from malicious Lua code, as well as control over which
    libraries are available to scripts.
  * Lua profiling support
  * "Context" or "Sandboxing" support.  There should be the ability to set the
    `_ENV` upvalue of a loaded chunk to a table other than `_G`, so that you can
    have different environments for different loaded chunks.
  * More fleshed out Lua API, there is some missing nice to have functionality
    not exposed like storing values in the registry, and manipulating `LuaTable`
    metatables.
  * Better API documentation and more examples.
  * Generally paying attention to performance and having benchmarks.

Additionally, there are ways I would like to change this API, once support lands
in rustc.  For example:

  * Once ATCs land, there should be a way to wrap callbacks based on argument
    and return signature, rather than calling Lua.pack / Lua.unpack inside the
    callback.  Until then, it is impossible to name the type of the function
    that would do the wrapping (See
    [this reddit discussion](http://www.reddit.com/r/rust/comments/5yujt6/))
  * Once variadic generics land in some form (tuple based variadic generics?),
    the plan is to completely eliminate the hlist macros.

It is also worth it to list some non-goals for the project:

  * Be a perfect zero cost wrapper over the Lua C API
  * Allow the user to do absolutely everything that the Lua C API might allow

## API stability or lack thereof

This library is very much Work In Progress, so there is a lot of API churn.  I
think the library MIGHT be stable and usable enough to realistically use in a
real project, but I cannot yet provide a stable API.  I currently follow
"pre-1.0 semver" (if such a thing exists), but there have been a large number of
API version bumps, and there will probably continue to be.  If you have a
dependency on rlua, you might want to consider adding a 0.x version bound.

## Safety and panics

My *goal* is complete safety, it should not be possible to cause undefined
behavior whatsoever with the API, even in edge cases.  There is, however, QUITE
a lot of unsafe code in this crate, and I would call the current safety level of
the crate "Work In Progress".  If you find the ability to cause UB with this API
*at all*, please file a bug report.

There are, however, a few ways to cause *panics* and even *aborts* with this
API.  I'm going to describe a lot of the finer points of panic handling in the
library here, but again this should all be taken currently to be "Work In
Progress".

Panic / abort considerations when using this API:

  * The API should be panic safe currently, whenever a panic is generated the
    Lua stack is cleared and the `Lua` instance should continue to be usable.
  * Panic unwinds in Rust callbacks should currently be handled correctly, the
    unwind is caught and carried across the Lua API boundary, and Lua code
    cannot catch rust panics.
  * There are a few panics marked "internal error" that should be impossible to
    trigger.  If you encounter one of these this is a bug.
  * The library internally calls lua_checkstack to ensure that there is
    sufficient stack space, and if the stack cannot be sufficiently grown this
    is a panic.  There should not be a way to cause this using the API, if you
    encounter this, it is a bug.
  * Currently the Lua allocator is the default C alloctor, and allocation
    failures may cause an *abort*.  This API only attempts to handle errors in
    Lua functions that can cause an error either directly or by running
    arbitrary Lua code, not functions that can cause memory errors.  This may
    either cause an abort inside Lua itself, or from rust if the error happens
    in a protected call.  If there is a memory error received from a protected
    call, we cannot assume anything about the state of rust code because there
    might have been a longjmp at any arbitrary point, so we are forced to abort.
  * Similarly to the above, if there is an internal error in a __gc metamethod,
    this may cause an abort.  This can be triggered in this API by panicking
    during drop of a custom userdata type, but this already can cause aborts in
    normal Rust anyway.
  * There are currently no recursion limits on callbacks.  This could cause one
    of two problems, either the API will run out of stack space and cause a
    panic in Rust, or more likely it will cause an internal `LUA_USE_APICHECK`
    abort, from exceeding LUAI_MAXCCALLS.
  * There are no checks on argument sizes, and I think you can cause an abort by
    providing a large enough `LuaVariadic`.

## Examples

There's sort of a guided tour of the API [here](examples/examples.rs).
