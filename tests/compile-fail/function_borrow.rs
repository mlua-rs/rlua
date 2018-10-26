extern crate rlua;

use rlua::{Function, Lua, Result};

fn main() {
    struct Test(i32);

    Lua::new().context(|lua| {
        let test = Test(0);

        let func = lua.create_function(|_, ()| -> Result<i32> {
            //~^ error: closure may outlive the current function, but it borrows `test`, which is owned by the current function
            Ok(test.0)
        });
    });
}
