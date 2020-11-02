use std::sync::{Arc, Mutex};

use rlua::{Lua, MetaMethod, Result, Table, UserData, UserDataMethods};

/// Test if a table of clones of an original Arc<Mutex<T>>
/// get updated when the original does.
#[test]
fn arc_mux_many_to_one() {
    #[derive(Debug)]
    struct Foo(i32);
    impl UserData for Foo {
        fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
            methods.add_meta_method(MetaMethod::ToString, |_ctx, this, ()| {
                Ok(this.0.to_string())
            });
        }
    }

    let original = Arc::new(Mutex::new(Foo(1)));
    let clones = (0..10).map(|_| original.clone()).collect::<Vec<_>>();
    println!("{:?}", clones);

    let lua = Lua::new();
    lua.context(|ctx| -> Result<()> {
        let globals = ctx.globals();

        let clones_table = ctx
            .create_table_from(
                clones
                    .into_iter()
                    .enumerate()
                    .map(|(idx, val)| (idx + 1, val)),
            )
            .unwrap();
        globals.set("clones", clones_table).unwrap();

        ctx.load(
            r#"
            print("clones", clones)
            print(clones[1])

            output = ""
            for _idx, val in ipairs(clones) do
                -- yes this is bad practice to concat strings like this...
                output = output .. tostring(val) .. " "
            end
        "#,
        )
        .exec()
        .unwrap();
        assert_eq!(
            globals.get::<_, String>("output").unwrap(),
            "1 1 1 1 1 1 1 1 1 1 ".to_string()
        );

        *original.lock().unwrap() = Foo(5);

        ctx.load(
            r#"
            output = ""
            for _idx, val in ipairs(clones) do
                output = output .. tostring(val) .. " "
            end
        "#,
        )
        .exec()
        .unwrap();
        assert_eq!(
            globals.get::<_, String>("output").unwrap(),
            "5 5 5 5 5 5 5 5 5 5 ".to_string()
        );

        Ok(())
    })
    .unwrap();
}

/// Test if a set of Arc<Mutex<T>>s are all updated
/// when any of them is updated.
///
/// Also drops the original Arc<Mutex> (just to make sure).
#[test]
fn arc_mux_many_to_many() {
    #[derive(Debug)]
    struct Foo(i32);
    impl UserData for Foo {
        fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
            // Please never do this in an actual library
            methods.add_meta_method_mut(MetaMethod::Call, |_ctx, this, ()| {
                this.0 += 1;
                Ok(this.0)
            });
        }
    }

    let lua = Lua::new();
    lua.context(|ctx| -> Result<()> {
        let globals = ctx.globals();

        // how high to count to
        let test_size = 10_000;
        // length of the test vec
        let test_length = 1_000;
        globals.set("test_size", test_size).unwrap();
        globals.set("test_length", test_length).unwrap();

        let foo = Arc::new(Mutex::new(Foo(0)));
        let clones = ctx
            .create_table_from((0..test_length).map(|idx| (idx + 1, foo.clone())))
            .unwrap();
        globals.set("clones", clones).unwrap();

        drop(foo);

        ctx.load(
            r#"
            for i = 1,test_size do
                local idx = math.random(1, test_length)
                assert(clones[idx]() == i)
            end
        "#,
        )
        .exec()
        .unwrap();

        let processed_clones: Table = globals.get("clones").unwrap();
        for idx in 1..=test_length {
            assert_eq!(
                processed_clones
                    .get::<_, Arc<Mutex<Foo>>>(idx)
                    .unwrap()
                    .lock()
                    .unwrap()
                    .0,
                test_size
            );
        }

        Ok(())
    })
    .unwrap();
}
