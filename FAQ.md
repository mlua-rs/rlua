# FAQ

### Why 'static lifetime is required when passing data to Lua as UserData?

> Since we don't know when Lua will get around to garbage collecting our userdata, we can't make any assumptions about the lifetimes of any references in the userdata.

* https://github.com/amethyst/rlua/issues/74#issuecomment-370152884
* https://github.com/amethyst/rlua/issues/20#issuecomment-405751583

### What is `scope` and why should I need it?

Normally, Rust types passed to `Lua` must be `Send`, because `Lua` itself is `Send`, and must be `'static`, because there is no way to tell when Lua might garbage collect them.  There is, however, a limited way to lift both of these restrictions.  You can call `Lua::scope` to create userdata types that do not have to be `Send`, and callback types that do not have to be `Send` OR `'static`.  However, after `scope` returns any `UserData` passed to Lua are invalidated (access from Lua will error).

### How can I store Lua values (like `Table`) outside of `Lua::context()`?

The Lua API in general doesn't give out references to Lua values - you can only interact with them indirectly throught the API.  In `rlua`, types like `rlua::Table` internally store an index into the current `Lua` stack, which is why they are only valid within the `context` callback.

There are some options for keeping longer references:

#### Store it in the registry.

The Lua registry is a global `Table` available for use through the API.  Add references using `Context::create_registry_value()` which returns a key which can be later used with `Context::registry_value()`, or provide your own key with `Context::set_named_registry_value()`/`Context::named_registry_value()`.

#### Store it with a `UserData` value

A Lua value can be attached with a `UserData` value using `AnyUserData::set_user_value()`/`AnyUserData::get_user_value()`.
