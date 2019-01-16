use std::sync::Arc;

use rlua::{Error, Lua, Nil, UserData};

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

        lua.set_memory_limit(Some(initial_memory + 10000));
        match f.call::<_, ()>(()) {
            Err(Error::MemoryError(_)) => {}
            something_else => panic!("did not trigger memory error: {:?}", something_else),
        }

        lua.set_memory_limit(None);
        f.call::<_, ()>(()).expect("should trigger no memory limit");
    });
}

#[test]
fn test_gc_control() {
    let lua = Lua::new();
    assert!(lua.gc_is_running());
    lua.gc_stop();
    assert!(!lua.gc_is_running());
    lua.gc_restart();
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

#[test]
fn test_gc_error() {
    Lua::new().context(|lua| {
        match lua
            .load(
                r#"
                val = nil
                table = {}
                setmetatable(table, {
                    __gc = function()
                        error("gcwascalled")
                    end
                })
                table = nil
                collectgarbage("collect")
            "#,
            )
            .exec()
        {
            Err(Error::GarbageCollectorError(_)) => {}
            Err(e) => panic!("__gc error did not result in correct error, instead: {}", e),
            Ok(()) => panic!("__gc error did not result in error"),
        }
    });
}
