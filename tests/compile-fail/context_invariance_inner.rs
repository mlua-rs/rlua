extern crate rlua;

use rlua::Lua;

fn main() {
    Lua::new().context(|lua1| {
        Lua::new().context(|lua2| {
            let t = lua2.create_table().unwrap();
            //~^ error: 8:21: 8:40: borrowed data escapes outside of closure [E0521]
            //~^^ error: 8:21: 8:40: borrowed data escapes outside of closure [E0521]
            lua1.globals().set("t", t).unwrap();
        });
    });
}
