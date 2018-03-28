extern crate rlua;

use std::panic::catch_unwind;

use rlua::*;

fn main() {
    let lua = Lua::new();
    let table = lua.create_table().unwrap();
    let _ = catch_unwind(move || table.set("a", "b").unwrap());
    //~^ error: the trait bound `std::cell::UnsafeCell<()>: std::panic::RefUnwindSafe` is not satisfied in `rlua::Lua`
}
