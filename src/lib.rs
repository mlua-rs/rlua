pub use mlua::*;

pub mod prelude {
    pub use mlua::prelude::*;
    pub use super::ToLua;
}

pub type Context<'lua> = &'lua Lua;

pub use mlua::IntoLua as ToLua;
/*
pub trait ToLua<'lua> {
    fn to_lua(self, context: Context<'lua>) -> mlua::Result<Value<'lua>>;
}

impl<'lua, T: IntoLua<'lua>> ToLua<'lua> for T {
    fn to_lua(self, context: Context<'lua>) -> mlua::Result<Value<'lua>> {
        self.into_lua(context)
    }
}
*/

