use std::{
    cell::Cell,
    sync::{Arc, Mutex},
};

use rlua::{Lua, MetaMethod, Result, Table, UserData, UserDataMethods};

use crossbeam::atomic::AtomicCell;

/// Test if a table of clones of an original Arc<Mutex<T>>
/// get updated when the original does.
#[test]
fn arc_mux_many_to_one() {
    #[derive(Debug, Default)]
    struct Foo(i32);
    impl UserData for Foo {
        fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
            methods.add_meta_method(MetaMethod::ToString, |_ctx, this, ()| {
                let out = this.0.to_string();
                println!("to string called: {}", &out);
                Ok(out)
            });
        }
    }

    let original = Arc::new(Mutex::new(Foo(1)));
    let clones = (0..10).map(|_| original.clone()).collect::<Vec<_>>();

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

        let mut lock = original.lock().unwrap();
        *lock = Foo(5);
        drop(lock);

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
    #[derive(Debug, Default)]
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

/// Make sure nothing gets dropped twice
#[test]
fn drop_twice() {
    #[derive(Debug)]
    struct DropTracker {
        id: String,
        drop_tracker: Arc<Mutex<String>>,
    };

    impl Drop for DropTracker {
        fn drop(&mut self) {
            // Push this ID to the drop tracker
            self.drop_tracker.lock().unwrap().push_str(&self.id);
        }
    }

    impl UserData for DropTracker {
        fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
            methods.add_meta_method_mut(MetaMethod::Call, |_ctx, _this, ()| {
                // no-op
                Ok(())
            });
        }
    }

    impl Default for DropTracker {
        fn default() -> Self {
            Self {
                id: String::from("default"),
                drop_tracker: Default::default(),
            }
        }
    }

    let drop_tracker = Arc::new(Mutex::new(String::new()));
    let lua = Lua::new();
    lua.context(|ctx| {
        let test_size = 1000;
        let globals = ctx.globals();

        let original = Arc::new(Mutex::new(DropTracker {
            id: String::from("original"),
            drop_tracker: drop_tracker.clone(),
        }));
        let clones = ctx
            .create_table_from((0..test_size).map(|idx| (idx + 1, original.clone())))
            .unwrap();
        globals.set("clones", clones).unwrap();

        ctx.load(
            r#"
            for _idx, clone in ipairs(clones) do
                clone()
            end
        "#,
        )
        .exec()
        .unwrap();

        // We still haven't dropped anything yet
        assert_eq!(*drop_tracker.lock().unwrap(), "");
        // Drop the original wrapper
        drop(original);
        // still haven't dropped the real value
        assert_eq!(*drop_tracker.lock().unwrap(), "");
    });

    // still hasn't been dropped!
    assert_eq!(*drop_tracker.lock().unwrap(), "");

    // And *now* it has been dropped
    drop(lua);
    assert_eq!(*drop_tracker.lock().unwrap(), "original");
}
