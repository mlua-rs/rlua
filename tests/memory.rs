use std::sync::Arc;

use rlua::{Error, Lua, Nil, RluaCompat, UserData};

#[cfg(not(rlua_luajit))] // Custom allocators for LuaJIT not available
#[test]
fn test_memory_limit() {
    let lua = Lua::new();
    let initial_memory = lua.used_memory();
    assert!(
        initial_memory > 0,
        "used_memory reporting is wrong, lua uses memory for stdlib"
    );

    lua.context(|ctx| {
        let f = ctx
            .load("local t = {}; for i = 1,10000 do t[i] = i end")
            .into_function()
            .unwrap();
        f.call::<_, ()>(()).expect("should trigger no memory limit");

        // It's not clear this is needed.  On Lua 5.1, we fail to allocate
        // memory in the following `f.call` before actually running the
        // function otherwise.
        lua.gc_collect().expect("should collect garbage");

        lua.set_memory_limit(initial_memory + 10000).unwrap();
        match f.call::<_, ()>(()) {
            Err(Error::MemoryError(_)) => {}
            something_else => panic!("did not trigger memory error: {:?}", something_else),
        }

        lua.set_memory_limit(usize::MAX).unwrap();
        f.call::<_, ()>(()).expect("should trigger no memory limit");
    });
}

#[test]
fn test_gc_control() {
    let lua = Lua::new();
    #[cfg(any(rlua_lua53, rlua_lua54))]
    assert!(lua.gc_is_running());
    lua.gc_stop();
    #[cfg(any(rlua_lua53, rlua_lua54))]
    assert!(!lua.gc_is_running());
    lua.gc_restart();
    #[cfg(any(rlua_lua53, rlua_lua54))]
    assert!(lua.gc_is_running());

    struct MyUserdata(Arc<()>);
    impl UserData for MyUserdata {}

    lua.context(|ctx| {
        let rc = Arc::new(());
        ctx.globals()
            .set(
                "userdata",
                ctx.create_userdata(MyUserdata(rc.clone())).unwrap(),
            )
            .unwrap();
        ctx.globals().set("userdata", Nil).unwrap();

        assert_eq!(Arc::strong_count(&rc), 2);
        lua.gc_collect().unwrap();
        lua.gc_collect().unwrap();
        assert_eq!(Arc::strong_count(&rc), 1);
    });
}
