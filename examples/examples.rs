extern crate rlua;

use std::f32;

use rlua::{Lua, Result, Function, Variadic, UserData, UserDataMethods, MetaMethod};

fn examples() -> Result<()> {
    // Create a Lua context with Lua::new().  Eventually, this will allow
    // further control on the lua std library, and will specifically allow
    // limiting Lua to a subset of "safe" functionality.

    let lua = Lua::new();

    // You can get and set global variables.  Notice that the globals table here
    // is a permanent reference to _G, and it is mutated behind the scenes as
    // lua code is loaded.  This API is based heavily around internal mutation
    // (just like lua itself).

    let globals = lua.globals();

    globals.set("string_var", "hello")?;
    globals.set("int_var", 42)?;

    assert_eq!(globals.get::<_, String>("string_var")?, "hello");
    assert_eq!(globals.get::<_, i64>("int_var")?, 42);

    // You can load and evaluate lua code.  The second parameter here gives the
    // chunk a better name when lua error messages are printed.

    lua.exec::<()>(
        r#"
            global = 'foo'..'bar'
        "#,
        Some("example code"),
    )?;
    assert_eq!(globals.get::<_, String>("global")?, "foobar");

    assert_eq!(lua.eval::<i32>("1 + 1", None)?, 2);
    assert_eq!(lua.eval::<bool>("false == false", None)?, true);
    assert_eq!(lua.eval::<i32>("return 1 + 2", None)?, 3);

    // You can create and manage lua tables

    let array_table = lua.create_table();
    array_table.set(1, "one")?;
    array_table.set(2, "two")?;
    array_table.set(3, "three")?;
    assert_eq!(array_table.len()?, 3);

    let map_table = lua.create_table();
    map_table.set("one", 1)?;
    map_table.set("two", 2)?;
    map_table.set("three", 3)?;
    let v: i64 = map_table.get("two")?;
    assert_eq!(v, 2);

    // You can pass values like Table back into Lua

    globals.set("array_table", array_table)?;
    globals.set("map_table", map_table)?;

    lua.eval::<()>(
        r#"
        for k, v in pairs(array_table) do
            print(k, v)
        end

        for k, v in pairs(map_table) do
            print(k, v)
        end
        "#,
        None,
    )?;

    // You can load lua functions

    let print: Function = globals.get("print")?;
    print.call::<_, ()>("hello from rust")?;

    // This API handles variadics using Heterogeneous Lists.  This is one way to
    // call a function with multiple parameters:

    print.call::<_, ()>(("hello", "again", "from", "rust"))?;

    // You can bind rust functions to lua as well

    let check_equal = lua.create_function(|lua, args| {
        // Functions wrapped in lua receive their arguments packed together as
        // MultiValue.  The first thing that most wrapped functions will do
        // is "unpack" this MultiValue into its parts.  Due to lifetime type
        // signature limitations, this cannot be done automatically from the
        // function signature, but this will be fixed with ATCs.  Notice the use
        // of the hlist macros again.
        let (list1, list2) = lua.unpack::<(Vec<String>, Vec<String>)>(args)?;

        // This function just checks whether two string lists are equal, and in
        // an inefficient way.  Results are returned with lua.pack, which takes
        // any number of values and turns them back into MultiValue.  In this
        // way, multiple values can also be returned to Lua.  Again, this cannot
        // be inferred as part of the function signature due to the same
        // lifetime type signature limitations.
        lua.pack(list1 == list2)
    });
    globals.set("check_equal", check_equal)?;

    // You can also accept variadic arguments to rust callbacks.

    let join = lua.create_function(|lua, args| {
        let strings = lua.unpack::<Variadic<String>>(args)?.0;
        // (This is quadratic!, it's just an example!)
        lua.pack(strings.iter().fold("".to_owned(), |a, b| a + b))
    });
    globals.set("join", join)?;

    assert_eq!(
        lua.eval::<bool>(
            r#"check_equal({"a", "b", "c"}, {"a", "b", "c"})"#,
            None,
        )?,
        true
    );
    assert_eq!(
        lua.eval::<bool>(
            r#"check_equal({"a", "b", "c"}, {"d", "e", "f"})"#,
            None,
        )?,
        false
    );
    assert_eq!(lua.eval::<String>(r#"join("a", "b", "c")"#, None)?, "abc");

    // You can create userdata with methods and metamethods defined on them.
    // Here's a worked example that shows many of the features of this API
    // together

    #[derive(Copy, Clone)]
    struct Vec2(f32, f32);

    impl UserData for Vec2 {
        fn add_methods(methods: &mut UserDataMethods<Self>) {
            methods.add_method("magnitude", |lua, vec, _: ()| {
                let mag_squared = vec.0 * vec.0 + vec.1 * vec.1;
                lua.pack(mag_squared.sqrt())
            });

            methods.add_meta_function(MetaMethod::Add, |lua, params| {
                let (vec1, vec2) = lua.unpack::<(Vec2, Vec2)>(params)?;
                lua.pack(Vec2(vec1.0 + vec2.0, vec1.1 + vec2.1))
            });
        }
    }

    let vec2_constructor = lua.create_function(|lua, args| {
        let (x, y) = lua.unpack::<(f32, f32)>(args)?;
        lua.pack(Vec2(x, y))
    });
    globals.set("vec2", vec2_constructor)?;

    assert!(
        lua.eval::<f32>(
            "(vec2(1, 2) + vec2(2, 2)):magnitude()",
            None,
        )? - 5.0 < f32::EPSILON
    );

    Ok(())
}

fn main() {
    examples().unwrap();
}
