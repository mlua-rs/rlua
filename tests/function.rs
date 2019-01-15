use rlua::{Function, Lua, String};

#[test]
fn test_function() {
    Lua::new().context(|lua| {
        let globals = lua.globals();
        lua.load(
            r#"
                function concat(arg1, arg2)
                    return arg1 .. arg2
                end
            "#,
        )
        .exec()
        .unwrap();

        let concat = globals.get::<_, Function>("concat").unwrap();
        assert_eq!(concat.call::<_, String>(("foo", "bar")).unwrap(), "foobar");
    });
}

#[test]
fn test_bind() {
    Lua::new().context(|lua| {
        let globals = lua.globals();
        lua.load(
            r#"
                function concat(...)
                    local res = ""
                    for _, s in pairs({...}) do
                        res = res..s
                    end
                    return res
                end
            "#,
        )
        .exec()
        .unwrap();

        let mut concat = globals.get::<_, Function>("concat").unwrap();
        concat = concat.bind("foo").unwrap();
        concat = concat.bind("bar").unwrap();
        concat = concat.bind(("baz", "baf")).unwrap();
        assert_eq!(
            concat.call::<_, String>(("hi", "wut")).unwrap(),
            "foobarbazbafhiwut"
        );
    });
}

#[test]
fn test_rust_function() {
    Lua::new().context(|lua| {
        let globals = lua.globals();
        lua.load(
            r#"
                function lua_function()
                    return rust_function()
                end

                -- Test to make sure chunk return is ignored
                return 1
            "#,
        )
        .exec()
        .unwrap();

        let lua_function = globals.get::<_, Function>("lua_function").unwrap();
        let rust_function = lua.create_function(|_, ()| Ok("hello")).unwrap();

        globals.set("rust_function", rust_function).unwrap();
        assert_eq!(lua_function.call::<_, String>(()).unwrap(), "hello");
    });
}
