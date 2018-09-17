extern crate rlua;

use rlua::Lua;

fn main() {
    struct Test {
        field: i32,
    }

    let lua = Lua::new();
    lua.scope(|scope| {
        let f = {
            let mut test = Test { field: 0 };

            scope
                .create_function_mut(|_, ()| {
                    test.field = 42;
                    //~^ error: `test` does not live long enough
                    Ok(())
                })
                .unwrap()
        };

        f.call::<_, ()>(()).unwrap();
    });
}
