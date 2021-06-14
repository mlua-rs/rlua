extern crate rlua;

use rlua::{Lua, Table};

fn main() {
    struct Test {
        field: i32,
    }

    Lua::new().context(|lua| {
        let mut outer: Option<Table> = None;
        lua.scope(|scope| {
            let f = scope
                //~^ error: borrowed data escapes outside of closure
                .create_function_mut(|_, t: Table| {
                    outer = Some(t);
                    //~^ error: `outer` does not live long enough
                    Ok(())
                })
                .unwrap();
            f.call::<_, ()>(lua.create_table()).unwrap();
        });
    });
}
