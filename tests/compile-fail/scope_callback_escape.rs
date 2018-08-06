extern crate rlua;

use rlua::*;

fn main() {
    struct Test {
        field: i32,
    }

    let lua = Lua::new();
    let mut outer: Option<Table> = None;
    lua.scope(|scope| {
        let f = scope
            .create_function_mut(|_, t: Table| {
                //~^^ error: borrowed data cannot be stored outside of its closure
                outer = Some(t);
                Ok(())
            })
            .unwrap();
        f.call::<_, ()>(lua.create_table()).unwrap();
    });
}
