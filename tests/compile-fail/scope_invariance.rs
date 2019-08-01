extern crate rlua;

use rlua::Lua;

fn main() {
    struct Test {
        field: i32,
    }

    Lua::new().context(|lua| {
        lua.scope(|scope| {
            let f = {
                let mut test = Test { field: 0 };

                scope
                    .create_function_mut(|_, ()| {
                        //~^ error: closure may outlive the current function, but it borrows `test`, which is owned by the current function
                        test.field = 42;
                        Ok(())
                    })
                    .unwrap()
            };

            f.call::<_, ()>(()).unwrap();
        });
    });
}
