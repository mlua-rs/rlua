use rlua::{Lua, Result};

fn main() -> Result<()> {
    let lua = Lua::new();
    lua.context(|lua_ctx| {
        // add `some_directory` to the package path
        lua_ctx.load("package.path = package.path .. ';./examples/some_directory/?.lua'").exec()?;

        // require a module located in the newly added directory
        lua_ctx.load("require'new_module'").exec()?;

        Ok(())
    })?;

    Ok(())
}
