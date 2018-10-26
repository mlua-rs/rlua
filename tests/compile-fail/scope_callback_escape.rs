extern crate rlua;

use rlua::{Lua, Table};

fn main() {
    struct Test {
        field: i32,
    }

    let lua = Lua::new();
    let mut outer: Option<Table> = None;
    lua.scope(|scope| {
        let f = scope
            .create_scoped_function_mut(|_, t: Table| {
                outer = Some(t);
                //~^^ error: borrowed data cannot be stored outside of its closure
                Ok(())
            })
            .unwrap();
        f.call::<_, ()>(scope.create_table()).unwrap();
    });
}
