# FAQ

### Why 'static lifetime is required when passing data to Lua as UserData?

> Since we don't know when Lua will get around to garbage collecting our userdata, we can't make any assumptions about the lifetimes of any references in the userdata.

* https://github.com/amethyst/rlua/issues/74#issuecomment-370152884
* https://github.com/amethyst/rlua/issues/20#issuecomment-405751583

### What is `scope` and why should I need it?

Normally, Rust types passed to `Lua` must be `Send`, because `Lua` itself is `Send`, and must be `'static`, because there is no way to tell when Lua might garbage collect them.  There is, however, a limited way to lift both of these restrictions.  You can call `Lua::scope` to create userdata types that do not have to be `Send`, and callback types that do not have to be `Send` OR `'static`.  However, after `scope` returns any `UserData` passed to Lua are invalidated (access from Lua will error).

### How to I store values along-side `Lua`?

> There are two rules about rlua reference types that you're butting up against:
>
> rlua reference types (Table, Function, etc) hold references to Lua, and are not really designed to be stored along-side Lua, because that would require self borrows. You CAN make structs in Rust that self borrow, but you really don't want to go down that road, it's very complex and you 99.999% of the time don't need it.
>
> rlua reference types are also not designed to be stored inside userdata inside Lua. When I say userdata here, I mean both actual UserData types and also things like Rust callbacks. For one, there are some actual safety issues if you were able to do that, but more importantly Lua itself is not really designed for this either. Lua's C API does not provide a way of telling Lua that a userdata contains a registry reference, so you would end up potentially making something that cannot be garbage collected. The actual issue is very complicated but you can read about it some here.

* https://github.com/kyren/rlua/issues/73#issuecomment-370222198
