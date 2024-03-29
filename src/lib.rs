pub use mlua::*;

pub mod prelude {
    pub use super::RluaCompat;
    pub use super::ToLua;
    pub use mlua::prelude::*;
}

pub type Context<'lua> = &'lua Lua;

pub trait RluaCompat {
    #[deprecated = "Context is no longer needed; call methods on Lua directly."]
    fn context<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&Lua) -> R;
}

impl RluaCompat for Lua {
    fn context<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&Lua) -> R,
    {
        f(self)
    }
}

pub use mlua::IntoLua as ToLua;

pub trait ToLuaCompat<'lua> {
    #[deprecated = "ToLua::to_lua has become IntoLua::into_lua"]
    fn to_lua(self, context: &'lua Lua) -> mlua::Result<Value<'lua>>;
}

impl<'lua, T: IntoLua<'lua>> ToLuaCompat<'lua> for T {
    fn to_lua(self, context: &'lua Lua) -> mlua::Result<Value<'lua>> {
        self.into_lua(context)
    }
}
