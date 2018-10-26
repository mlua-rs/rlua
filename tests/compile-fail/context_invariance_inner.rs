extern crate rlua;

use rlua::Lua;

fn main() {
    Lua::new().scope(|lua1| {
        Lua::new().scope(|lua2| {
            let t = lua2.create_table().unwrap();
            //~^ error: cannot infer an appropriate lifetime for lifetime parameter `'lua` due to
            // conflicting requirements
            lua1.globals().set("t", t).unwrap();
        });
    });
}
