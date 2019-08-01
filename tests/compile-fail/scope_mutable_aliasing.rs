extern crate rlua;

use rlua::{Lua, UserData};

fn main() {
    struct MyUserData<'a>(&'a mut i32);
    impl<'a> UserData for MyUserData<'a> {};

    let mut i = 1;

    Lua::new().context(|lua| {
        lua.scope(|scope| {
            let a = scope.create_nonstatic_userdata(MyUserData(&mut i)).unwrap();
            let b = scope.create_nonstatic_userdata(MyUserData(&mut i)).unwrap();
            //~^ error: cannot borrow `i` as mutable more than once at a time
        });
    });
}
