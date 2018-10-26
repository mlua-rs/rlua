extern crate rlua;

use rlua::{Lua, Table};

fn main() {
    struct Test {
        field: i32,
    }

    let lua = Lua::new();
    lua.scope(|scope| {
        let mut inner: Option<Table> = None;
        let f = scope
            .create_scoped_function_mut(|_, t: Table| {
                //~^ error: cannot infer an appropriate lifetime for lifetime parameter `'lua` due
                // to conflicting requirements
                inner = Some(t);
                Ok(())
            })
            .unwrap();
        f.call::<_, ()>(scope.create_table()).unwrap();
    });
}
