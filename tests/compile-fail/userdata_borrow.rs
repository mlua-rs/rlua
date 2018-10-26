extern crate rlua;

use rlua::{AnyUserData, Lua, Table, UserData};

fn main() {
    Lua::new().context(|lua| {
        let globals = lua.globals();

        // Should not allow userdata borrow to outlive lifetime of AnyUserData handle
        struct MyUserData;
        impl UserData for MyUserData {};
        let userdata_ref;
        {
            let touter = globals.get::<_, Table>("touter").unwrap();
            touter
                .set("userdata", lua.create_userdata(MyUserData).unwrap())
                .unwrap();
            let userdata = touter.get::<_, AnyUserData>("userdata").unwrap();
            userdata_ref = userdata.borrow::<MyUserData>();
            //~^ error: `userdata` does not live long enough
        }
    });
}
