# rlua -- High level bindings between Rust and Lua

[![Build Status](https://img.shields.io/circleci/project/github/amethyst/rlua.svg)](https://circleci.com/gh/amethyst/rlua)
[![Latest Version](https://img.shields.io/crates/v/rlua.svg)](https://crates.io/crates/rlua)
[![API Documentation](https://docs.rs/rlua/badge.svg)](https://docs.rs/rlua)

[Guided Tour](examples/guided_tour.rs)

This library is a high level interface between Rust and Lua.  Its goal is to be
an easy to use, practical, flexible, and *safe* API between Rust and Lua.

`rlua` is NOT designed to be a perfect zero cost wrapper over the Lua C API,
because such a wrapper cannot maintain the safety guarantees that `rlua` is
designed to have.  Every place where the Lua C API may trigger an error longjmp
in any way is protected by `lua_pcall`, and the user of the library is protected
from directly interacting with unsafe things like the Lua stack, and there is
overhead associated with this safety.  However, performance *is* a focus of the
library to the extent possible while maintaining safety, so if you encounter
something that is egregiously worse than using the Lua C API directly, or simply
something you feel could perform better, feel free to file a bug report.

## API stability

Currently, this library follows a pre-1.0 semver, so all API changes should be
accompanied by 0.x version bumps.  See the [Version 1.0
milestone](https://github.com/amethyst/rlua/milestone/1) for the work planned
to be done before a more stable 1.0 release.  There may be breaking changes as
these issues are dealt with on the way (the version number will be bumped as
needed).

## Lua versions supported

As of release 0.18, the version of Lua can be configured at build time using
Cargo features.  Lua 5.4 is the default.  The rlua API stays the same with
different Lua versions, though there are a small number of limitations.  Lua
code may, of course, behave a little differently between the versions.

Only one can be selected at a time, so to select anything
other than the default (built-in Lua 5.4) you will need to disable default
features.

The available features are:

| Cargo feature | Lua version |
| ------------- | ----------- |
| builtin-lua54 | Lua 5.4 (source included in package, default) |
| builtin-lua53 | Lua 5.3 (source included in package) |
| builtin-lua51 | Lua 5.1 (source included in package) |
| system-lua54 | Lua 5.4 (installed on host system, found using pkg-config) |
| system-lua53 | Lua 5.3 (installed on host system, found using pkg-config) |
| system-lua51 | Lua 5.1 (installed on host system, found using pkg-config) |

At current writing rlua has not been tested with alternative Lua
implementations (such as Luajit) which share PUC-Rio Lua's C API, but it is
expected that they can be made to work with little if any change to rlua, and
support would be welcome.

## Safety and Panics

The goal of this library is complete safety by default: it should not be
possible to cause undefined behavior with the safe API, even in edge cases.
Unsoundness is considered the most serious kind of bug, so if you find the
ability to cause UB with this API without `unsafe`, please file a bug report.

This includes calling functions in the Lua standard library; some unsafe
functions are wrapped by default (for example to prevent loading binary
modules), but these wrappers can be disabled using one of the `unsafe`
constructors for the `Lua` object if required for the application.

Another goal of this library is complete protection from panics: currently, it
should not be possible for a script to trigger a panic.  There ARE however
several internal panics in the library, but triggering them is considered a bug.
If you find a way to trigger these internal panics, please file a bug report.

Yet another goal of the library is to, in all cases, safely handle panics that
are generated inside Rust callbacks.  Panic unwinds in Rust callbacks should
currently be handled correctly -- the unwind is caught and carried across the
Lua API boundary as a regular Lua error in a way that prevents Lua from catching
it.  This is done by overriding the normal Lua 'pcall' and 'xpcall' functions
with custom versions that cannot catch errors that are actually from Rust
panics, and by handling panic errors on the receiving Rust side by resuming the
panic.

`rlua` should also be panic safe in another way as well, which is that any `Lua`
instances or handles should remain usable after a user generated panic, and such
panics should not break internal invariants or leak Lua stack space.  This is
mostly important to safely use `rlua` types in Drop impls, as you should not be
using panics for general error handling.

In summary, here is a list of `rlua` behaviors that should be considered a bug.
If you encounter them, a bug report would be very welcome:

  * If you can cause UB with `rlua` without typing the word "unsafe", this is a
    bug.
  * If your program panics with a message that contains the string "rlua
    internal error", this is a bug.
  * The above is true even for the internal panic about running out of stack
    space!  There are a few ways to generate normal script errors by running out
    of stack, but if you encounter a *panic* based on running out of stack, this
    is a bug.
  * When the internal version of Lua is built using the `cc` crate, and
    `cfg!(debug_assertions)` is true, Lua is built with the `LUA_USE_APICHECK`
    define set.  Any abort caused by this internal Lua API checking is
    definitely a bug, and is likely to be a soundness bug because without
    `LUA_USE_APICHECK` it would likely instead be UB.
  * Lua C API errors are handled by longjmp.  All instances where the Lua C API
    would otherwise longjmp over calling stack frames should be guarded against,
    except in internal callbacks where this is intentional.  If you detect that
    `rlua` is triggering a longjmp over your Rust stack frames, this is a bug!
  * If you can somehow handle a panic triggered from a Rust callback in Lua,
    this is a bug.
  * If you detect that, after catching a panic or during a Drop triggered from a
    panic, a `Lua` or handle method is triggering other bugs or there is a Lua
    stack space leak, this is a bug.  `rlua` instances are supposed to remain
    fully usable in the face of user generated panics.  This guarantee does not
    extend to panics marked with "rlua internal error" simply because that is
    already indicative of a separate bug.

## Sandboxing and Untrusted Scripts

The API now contains the pieces necessary to implement simple, limited
"sandboxing" of Lua scripts by controlling their environment, limiting their
allotted VM instructions, and limiting the amount of memory they may allocate.

These features deserve a few words of warning: **Do not use them to run
untrusted scripts unless you really Know What You Are Doing (tm)** (and even
then, you probably should not do this).

First, this library contains a huge amount of unsafe code, and I currently
*would not trust it in a truly security sensitive context*.  There are almost
certainly bugs still lurking in this library!  It is surprisingly, fiendishly
difficult to use the Lua C API without the potential for unsafety.

Second, properly sandboxing Lua scripts can be quite difficult, much of the
stdlib is unsafe, and sometimes in surprising ways.  Some information on this
can be found [here](http://lua-users.org/wiki/SandBoxes).

Third, PUC-Rio Lua is a C library not *really* designed to be used with
untrusted scripts.  Please understand that though PUC-Rio Lua is an extremely
well written language runtime, it is still quite a lot of C code, and it is not
commonly used with truly malicious scripts.  Take a look
[here](https://www.lua.org/bugs.html) and count how many bugs resulted in memory
unsafety in the interpreter.  Another small example: did you know there is a way
to attack Lua tables to cause linear complexity in the table length operator?
That this still counts as one VM instruction?

Fourth, if you provide a callback API to scripts, it can be very difficult to
secure that API.  Do all of your API functions have some maximum runtime?  Do
any of your API functions allow the script to allocate via Rust?  Are there
limits on how much they can allocate this way?  All callback functions still
count as a single VM instruction!

In any case, sandboxing in this way may still be useful to protect against buggy
(but non-malicious) scripts, and may even serve as a single *layer* of a larger
security strategy, but **please think twice before relying on this to protect
you from untrusted Lua code**.

## License

This project is licensed under the [MIT license](LICENSE)
