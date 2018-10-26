extern crate rlua;

use std::panic::catch_unwind;

use rlua::Lua;

fn main() {
    Lua::new().scope(|lua| {
        let context = lua.context();
        catch_unwind(move || {
            //~^ error: the type `std::cell::UnsafeCell<()>` may contain interior mutability and a reference
            // may not be safely transferrable across a catch_unwind boundary
            context.create_table().unwrap();
        });
    });
}
