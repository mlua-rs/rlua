extern crate rlua;

use rlua::Lua;

fn main() {
    Lua::new().context(|lua1| {
        let t = lua1.create_table().unwrap();
        //~^ error: borrowed data escapes outside of closure [E0521]
        Lua::new().context(|lua2| {
            lua2.globals().set("t", t).unwrap();
            //~^ error: borrowed data escapes outside of closure [E0521]
        });
    });
}
