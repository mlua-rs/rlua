extern crate rlua;

use rlua::Lua;

fn main() {
    Lua::new().context(|lua1| {
        let t = lua1.create_table().unwrap();
        //~^ error: cannot infer an appropriate lifetime for lifetime parameter `'lua` due to
        // conflicting requirements
        Lua::new().context(|lua2| {
            lua2.globals().set("t", t).unwrap();
        });
    });
}
