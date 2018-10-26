extern crate rlua;

use rlua::Lua;

fn main() {
    Lua::new().context(|lua1| {
        Lua::new().context(|lua2| {
            let t = lua2.create_table().unwrap();
            //~^ error: cannot infer an appropriate lifetime for lifetime parameter `'lua` due to
            // conflicting requirements
            lua1.globals().set("t", t).unwrap();
        });
    });
}
