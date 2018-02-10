extern crate rlua;

use rlua::*;

fn main() {
    let lua = Lua::new();
    struct Test(i32);

    let test = Test(0);

    let func = lua.create_function(|_, ()| -> Result<i32> {
        //~^ error: closure may outlive the current function, but it borrows `test`, which is owned by the current function
        Ok(test.0)
    });
}
