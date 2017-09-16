extern crate rlua;

use rlua::*;

fn main() {
    let lua = Lua::new();

    // Should not allow self borrow of lua, it can change addresses
    let func = lua.create_function(|_, ()| -> Result<i32> {
        //~^ error: closure may outlive the current function, but it borrows `lua`, which is owned by the current function
        lua.eval::<i32>("1 + 1", None)
    });
}
