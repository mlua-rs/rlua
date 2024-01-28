extern crate rlua;

use std::panic::catch_unwind;

use rlua::{Lua, RluaCompat};

fn main() {
    let lua = Lua::new();
    catch_unwind(|| {
        //~^ error: the type `UnsafeCell<()>` may contain interior mutability and a reference
        // may not be safely transferrable across a catch_unwind boundary
        lua.context(|lua| {
            lua.create_table().unwrap();
        });
    });
}
