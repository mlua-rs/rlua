extern crate rlua;

use std::panic::catch_unwind;

use rlua::{Lua, RluaCompat};

fn main() {
    Lua::new().context(|lua| {
        catch_unwind(move || {
            //~^ error: the type `UnsafeCell<()>` may contain interior mutability and a reference
            // may not be safely transferrable across a catch_unwind boundary
            lua.create_table().unwrap();
        });
    });
}
