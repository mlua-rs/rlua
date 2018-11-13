extern crate rlua;

use std::cell::RefCell;

use rlua::{Lua, Table};

fn main() {
    thread_local! {
        static BAD_TIME: RefCell<Option<Table<'static>>> = RefCell::new(None);
    }

    let lua = Box::leak(Box::new(Lua::new()));
    lua.context(|lua| {
        lua.create_function(|_, table: Table| {
            //~^ error: cannot infer an appropriate lifetime for lifetime parameter `'lua` due to
            // conflicting requirements
            BAD_TIME.with(|bt| {
                *bt.borrow_mut() = Some(table);
            });
            Ok(())
        })
        .unwrap()
        .call::<_, ()>(lua.create_table().unwrap())
        .unwrap();
    });
}
