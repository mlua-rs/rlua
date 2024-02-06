use std::panic::catch_unwind;

use rlua::{Error, Function, Lua, Result, RluaCompat, Thread, ThreadStatus};

#[test]
fn test_thread() {
    Lua::new().context(|lua| {
        let thread = lua
            .create_thread(
                lua.load(
                    r#"
                        function (s)
                            local sum = s
                            for i = 1,4 do
                                sum = sum + coroutine.yield(sum)
                            end
                            return sum
                        end
                    "#,
                )
                .eval()
                .unwrap(),
            )
            .unwrap();

        assert_eq!(thread.status(), ThreadStatus::Resumable);
        assert_eq!(thread.resume::<_, i64>(0).unwrap(), 0);
        assert_eq!(thread.status(), ThreadStatus::Resumable);
        assert_eq!(thread.resume::<_, i64>(1).unwrap(), 1);
        assert_eq!(thread.status(), ThreadStatus::Resumable);
        assert_eq!(thread.resume::<_, i64>(2).unwrap(), 3);
        assert_eq!(thread.status(), ThreadStatus::Resumable);
        assert_eq!(thread.resume::<_, i64>(3).unwrap(), 6);
        assert_eq!(thread.status(), ThreadStatus::Resumable);
        assert_eq!(thread.resume::<_, i64>(4).unwrap(), 10);
        assert_eq!(thread.status(), ThreadStatus::Unresumable);

        let accumulate = lua
            .create_thread(
                lua.load(
                    r#"
                        function (sum)
                            while true do
                                sum = sum + coroutine.yield(sum)
                            end
                        end
                    "#,
                )
                .eval::<Function>()
                .unwrap(),
            )
            .unwrap();

        for i in 0..4 {
            accumulate.resume::<_, ()>(i).unwrap();
        }
        assert_eq!(accumulate.resume::<_, i64>(4).unwrap(), 10);
        assert_eq!(accumulate.status(), ThreadStatus::Resumable);
        assert!(accumulate.resume::<_, ()>("error").is_err());
        assert_eq!(accumulate.status(), ThreadStatus::Error);

        let thread = lua
            .load(
                r#"
                    coroutine.create(function ()
                        while true do
                            coroutine.yield(42)
                        end
                    end)
                "#,
            )
            .eval::<Thread>()
            .unwrap();
        assert_eq!(thread.status(), ThreadStatus::Resumable);
        assert_eq!(thread.resume::<_, i64>(()).unwrap(), 42);

        let thread: Thread = lua
            .load(
                r#"
                    coroutine.create(function(arg)
                        assert(arg == 42)
                        local yieldarg = coroutine.yield(123)
                        assert(yieldarg == 43)
                        return 987
                    end)
                "#,
            )
            .eval()
            .unwrap();

        assert_eq!(thread.resume::<_, u32>(42).unwrap(), 123);
        assert_eq!(thread.resume::<_, u32>(43).unwrap(), 987);

        match thread.resume::<_, u32>(()) {
            Err(Error::CoroutineInactive) => {}
            Err(_) => panic!("resuming dead coroutine error is not CoroutineInactive kind"),
            _ => panic!("resuming dead coroutine did not return error"),
        }
    });
}

// Before Lua 5.3, coroutine.create() could only take a Lua function.
#[cfg(any(rlua_lua53, rlua_lua54))]
#[test]
fn coroutine_from_closure() {
    Lua::new().context(|lua| {
        let thrd_main = lua.create_function(|_, ()| Ok(())).unwrap();
        lua.globals().set("main", thrd_main).unwrap();
        let thrd: Thread = lua.load("coroutine.create(main)").eval().unwrap();
        thrd.resume::<_, ()>(()).unwrap();
    });
}

#[test]
fn coroutine_panic() {
    match catch_unwind(|| -> Result<()> {
        // check that coroutines propagate panics correctly
        Lua::new().context(|lua| {
            let thrd_main = lua.create_function(|_, ()| -> Result<()> {
                panic!("test_panic");
            })?;
            lua.globals().set("main", thrd_main.clone())?;
            let thrd: Thread = lua.create_thread(thrd_main)?;
            thrd.resume(())
        })
    }) {
        Ok(r) => panic!("coroutine panic not propagated, instead returned {:?}", r),
        Err(p) => assert!(*p.downcast::<&str>().unwrap() == "test_panic"),
    }
}
