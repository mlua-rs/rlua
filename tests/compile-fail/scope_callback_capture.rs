extern crate rlua;

use rlua::{Lua, Table};

fn main() {
    struct Test {
        field: i32,
    }

    Lua::new().context(|lua| {
        lua.scope(|scope| {
            let mut inner: Option<Table> = None;
            let f = scope
                .create_function_mut(move |lua, t: Table| {
                    //~^ error: cannot infer an appropriate lifetime for autoref due to conflicting
                    // requirements
                    if let Some(old) = inner.take() {
                        // Access old callback `Lua`.
                    }
                    inner = Some(t);
                    Ok(())
                })
                .unwrap();
            f.call::<_, ()>(lua.create_table()).unwrap();
        });
    });
}
