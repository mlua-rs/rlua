# rlua -- High level bindings between Rust and Lua

This library is a WIP high level interface between Rust and Lua.  Its major goal
is to expose as flexible of an API between Rust and Lua as possible, while also
being completely safe.

There are other high level lua bindings systems for rust, and this crate is an
exploration of a different part of the design space.  The main high level
interface to Lua right now is [hlua](https://github.com/tomaka/hlua/) which you
should definitely check out and use if it suits your needs.  This crate has the
following differences with hlua:

  * Handles to Lua values use the Lua registry, not the stack
  * Handles to Lua values are all internally mutable
  * Handles to Lua values use non-mutable borrows the main Lua object, so
    there can be multiple handles or long lived handles
  * Targets lua 5.3

The key difference here is that rlua handles rust-side references to Lua values
in a fundamentally different way than hlua, more similar to other lua bindings
systems like [Selene](https://github.com/jeremyong/Selene) for C++.  Values like
LuaTable and LuaFunction that hold onto Lua values in the Rust stack, instead of
pointing at values in the Lua stack, are placed into the registry with luaL_ref.
In this way, it is possible to have an arbitrary number of handles to internal
Lua values at any time, created and destroyed in arbitrary order.  This approach
IS slightly slower than the approach that hlua takes of only manipulating the
lua stack, but this, combined with internal mutability, allows for a much more
flexible API.

Currently exposes a *somewhat* complete Lua API covering values and tables and
functions and userdata, but does not yet cover coroutines.  This API is actually
heavily inspired by the lua API that I previously wrote for Starbound, and will
become feature complete with that API over time.  Some capabilities that API has
that are on the roadmap:

  * Proper coroutine support
  * Lua profiling support
  * Execution limits like total instruction limits or lua <-> rust recursion
    limits
  * Security limits on the lua stdlib, and general control over the loaded
    lua libraries.
  * "Context" or "Sandboxing" support, this was probably a bit too heavyweight
    in Starbound's API, but there will be the ability to set the `_ENV` upvalue
    of a loaded chunk to a table other than `_G`, so that you can have different
    environments for different loaded chunks.

There are also some more general things that need to be done:

  * More fleshed out Lua API, things like custom metatables and exposing the
    registry.
  * MUCH better API documentation, the current API documentation is almost
    non-existent.
  * Performance testing.

Additionally, there are ways I would like to change this API, once support lands
in rustc.  For example:

  * Once ATCs land, there should be a way to wrap callbacks based on argument
    and return signature, rather than calling lua.pack / lua.unpack inside the
    callback.  Until then, it is impossible to name the type of the function
    that would do the wrapping.
  * Once tuple based variadic generics land, the plan is to completely
    eliminate the lua multi macros in favor of simple tuples.
 
See [this reddit discussion](http://www.reddit.com/r/rust/comments/5yujt6/) for
details of the current lifetime problem with callback wrapping.

## Safety

My *goal* is complete safety, it should not be possible to cause undefined
behavior whatsoever with the API, even in edge cases.  There is, however, QUITE
a lot of unsafe code in this crate, and I would call the current safety level
of the crate "Work In Progress".  The GOAL is for the crate to handle tricky
situations such as:

  * Panic safety, and carrying the panic across the lua api correctly
  * Lua stack size checking, and correctly handling lua being out of stack
    space
  * Leaving the correct elements on the lua stack and in the correct order,
    and panicking if these invariants are not met (due to internal bugs).
  * Correctly guarding the metatables of userdata so that scripts cannot, for
    example, swap the `__gc` methods around and cause UB.

The library currently attempts to handle each of these situations, but there
are so many ways to cause unsafety with Lua that it just needs more testing.

## Examples
Please look at the examples [here](examples/examples.rs).
