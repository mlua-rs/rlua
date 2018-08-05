extern crate rlua;

use std::panic::catch_unwind;

use rlua::*;

fn main() {
    let lua = Lua::new();
    let _ = catch_unwind(|| lua.create_table().unwrap());
    //~^ error: the type `std::cell::UnsafeCell<()>` may contain interior mutability and a reference
    // may not be safely transferrable across a catch_unwind boundary
}
