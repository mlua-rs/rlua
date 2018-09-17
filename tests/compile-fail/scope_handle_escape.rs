extern crate rlua;

use rlua::{Function, Lua};

fn main() {
    struct Test {
        field: i32,
    }

    let lua = Lua::new();
    let mut outer: Option<Function> = None;
    lua.scope(|scope| {
        let f = scope.create_function_mut(|_, ()| Ok(())).unwrap();
        outer = Some(scope.create_function_mut(|_, ()| Ok(())).unwrap());
        //~^ error: borrowed data cannot be stored outside of its closure
    });
}
