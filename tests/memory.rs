use rlua::{Error, Lua};

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
