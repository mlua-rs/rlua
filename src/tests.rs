use std::fmt;
use std::error;
use std::panic::catch_unwind;
use std::os::raw::c_void;

use String as LuaString;
use {
    Lua, Result, LuaExternalError, LightUserData, UserDataMethods, UserData, Table, Thread,
    ThreadStatus, Error, Function, Value, Variadic, MetaMethod
};

#[test]
fn test_set_get() {
    let lua = Lua::new();
    let globals = lua.globals();
    globals.set("foo", "bar").unwrap();
    globals.set("baz", "baf").unwrap();
    assert_eq!(globals.get::<_, String>("foo").unwrap(), "bar");
    assert_eq!(globals.get::<_, String>("baz").unwrap(), "baf");
}

#[test]
fn test_load() {
    let lua = Lua::new();
    let func = lua.load("return 1+2", None).unwrap();
    let result: i32 = func.call(()).unwrap();
    assert_eq!(result, 3);

    assert!(lua.load("§$%§&$%&", None).is_err());
}

#[test]
fn test_exec() {
    let lua = Lua::new();
    let globals = lua.globals();
    lua.exec::<()>(
        r#"
            res = 'foo'..'bar'
        "#,
        None,
    ).unwrap();
    assert_eq!(globals.get::<_, String>("res").unwrap(), "foobar");

    let module: Table = lua.exec(
        r#"
            local module = {}

            function module.func()
                return "hello"
            end

            return module
        "#,
        None,
    ).unwrap();
    assert!(module.contains_key("func").unwrap());
    assert_eq!(
        module
            .get::<_, Function>("func")
            .unwrap()
            .call::<_, String>(())
            .unwrap(),
        "hello"
    );
}

#[test]
fn test_eval() {
    let lua = Lua::new();
    assert_eq!(lua.eval::<i32>("1 + 1", None).unwrap(), 2);
    assert_eq!(lua.eval::<bool>("false == false", None).unwrap(), true);
    assert_eq!(lua.eval::<i32>("return 1 + 2", None).unwrap(), 3);
    match lua.eval::<()>("if true then", None) {
        Err(Error::IncompleteStatement(_)) => {}
        r => panic!("expected IncompleteStatement, got {:?}", r),
    }
}

#[test]
fn test_table() {
    let lua = Lua::new();
    let globals = lua.globals();

    globals.set("table", lua.create_table()).unwrap();
    let table1: Table = globals.get("table").unwrap();
    let table2: Table = globals.get("table").unwrap();

    table1.set("foo", "bar").unwrap();
    table2.set("baz", "baf").unwrap();

    assert_eq!(table2.get::<_, String>("foo").unwrap(), "bar");
    assert_eq!(table1.get::<_, String>("baz").unwrap(), "baf");

    lua.exec::<()>(
        r#"
            table1 = {1, 2, 3, 4, 5}
            table2 = {}
            table3 = {1, 2, nil, 4, 5}
        "#,
        None,
    ).unwrap();

    let table1 = globals.get::<_, Table>("table1").unwrap();
    let table2 = globals.get::<_, Table>("table2").unwrap();
    let table3 = globals.get::<_, Table>("table3").unwrap();

    assert_eq!(table1.len().unwrap(), 5);
    assert_eq!(
        table1
            .clone()
            .pairs()
            .collect::<Result<Vec<(i64, i64)>>>()
            .unwrap(),
        vec![(1, 1), (2, 2), (3, 3), (4, 4), (5, 5)]
    );
    assert_eq!(
        table1
            .clone()
            .sequence_values()
            .collect::<Result<Vec<i64>>>()
            .unwrap(),
        vec![1, 2, 3, 4, 5]
    );

    assert_eq!(table2.len().unwrap(), 0);
    assert_eq!(
        table2
            .clone()
            .pairs()
            .collect::<Result<Vec<(i64, i64)>>>()
            .unwrap(),
        vec![]
    );
    assert_eq!(
        table2
            .sequence_values()
            .collect::<Result<Vec<i64>>>()
            .unwrap(),
        vec![]
    );

    // sequence_values should only iterate until the first border
    assert_eq!(
        table3
            .sequence_values()
            .collect::<Result<Vec<i64>>>()
            .unwrap(),
        vec![1, 2]
    );

    globals
        .set(
            "table4",
            lua.create_sequence_from(vec![1, 2, 3, 4, 5]).unwrap(),
        )
        .unwrap();
    let table4 = globals.get::<_, Table>("table4").unwrap();
    assert_eq!(
        table4
            .pairs()
            .collect::<Result<Vec<(i64, i64)>>>()
            .unwrap(),
        vec![(1, 1), (2, 2), (3, 3), (4, 4), (5, 5)]
    );
}

#[test]
fn test_function() {
    let lua = Lua::new();
    let globals = lua.globals();
    lua.exec::<()>(
        r#"
            function concat(arg1, arg2)
                return arg1 .. arg2
            end
        "#,
        None,
    ).unwrap();

    let concat = globals.get::<_, Function>("concat").unwrap();
    assert_eq!(
        concat.call::<_, String>(hlist!["foo", "bar"]).unwrap(),
        "foobar"
    );
}

#[test]
fn test_bind() {
    let lua = Lua::new();
    let globals = lua.globals();
    lua.exec::<()>(
        r#"
            function concat(...)
                local res = ""
                for _, s in pairs({...}) do
                    res = res..s
                end
                return res
            end
        "#,
        None,
    ).unwrap();

    let mut concat = globals.get::<_, Function>("concat").unwrap();
    concat = concat.bind("foo").unwrap();
    concat = concat.bind("bar").unwrap();
    concat = concat.bind(hlist!["baz", "baf"]).unwrap();
    assert_eq!(
        concat.call::<_, String>(hlist!["hi", "wut"]).unwrap(),
        "foobarbazbafhiwut"
    );
}

#[test]
fn test_rust_function() {
    let mut captured_var = 2;
    {
        let lua = Lua::new();
        let globals = lua.globals();
        lua.exec::<()>(
            r#"
                function lua_function()
                    return rust_function()
                end

                -- Test to make sure chunk return is ignored
                return 1
            "#,
            None,
        ).unwrap();

        let lua_function = globals.get::<_, Function>("lua_function").unwrap();
        let rust_function = lua.create_function(|lua, _| {
            captured_var = 42;
            lua.pack("hello")
        });

        globals.set("rust_function", rust_function).unwrap();
        assert_eq!(lua_function.call::<_, String>(()).unwrap(), "hello");
    }
    assert_eq!(captured_var, 42);
}

#[test]
fn test_user_data() {
    struct UserData1(i64);
    struct UserData2(Box<i64>);

    impl UserData for UserData1 {};
    impl UserData for UserData2 {};

    let lua = Lua::new();

    let userdata1 = lua.create_userdata(UserData1(1));
    let userdata2 = lua.create_userdata(UserData2(Box::new(2)));

    assert!(userdata1.is::<UserData1>());
    assert!(!userdata1.is::<UserData2>());
    assert!(userdata2.is::<UserData2>());
    assert!(!userdata2.is::<UserData1>());

    assert_eq!(userdata1.borrow::<UserData1>().unwrap().0, 1);
    assert_eq!(*userdata2.borrow::<UserData2>().unwrap().0, 2);
}

#[test]
fn test_methods() {
    struct MyUserData(i64);

    impl UserData for MyUserData {
        fn add_methods(methods: &mut UserDataMethods<Self>) {
            methods.add_method("get_value", |lua, data, _| lua.pack(data.0));
            methods.add_method_mut("set_value", |lua, data, args| {
                data.0 = lua.unpack(args)?;
                lua.pack(())
            });
        }
    }

    let lua = Lua::new();
    let globals = lua.globals();
    let userdata = lua.create_userdata(MyUserData(42));
    globals.set("userdata", userdata.clone()).unwrap();
    lua.exec::<()>(
        r#"
            function get_it()
                return userdata:get_value()
            end

            function set_it(i)
                return userdata:set_value(i)
            end
        "#,
        None,
    ).unwrap();
    let get = globals.get::<_, Function>("get_it").unwrap();
    let set = globals.get::<_, Function>("set_it").unwrap();
    assert_eq!(get.call::<_, i64>(()).unwrap(), 42);
    userdata.borrow_mut::<MyUserData>().unwrap().0 = 64;
    assert_eq!(get.call::<_, i64>(()).unwrap(), 64);
    set.call::<_, ()>(100).unwrap();
    assert_eq!(get.call::<_, i64>(()).unwrap(), 100);
}

#[test]
fn test_metamethods() {
    #[derive(Copy, Clone)]
    struct MyUserData(i64);

    impl UserData for MyUserData {
        fn add_methods(methods: &mut UserDataMethods<Self>) {
            methods.add_method("get", |lua, data, _| lua.pack(data.0));
            methods.add_meta_function(MetaMethod::Add, |lua, args| {
                let hlist_pat![lhs, rhs] = lua.unpack::<HList![MyUserData, MyUserData]>(args)?;
                lua.pack(MyUserData(lhs.0 + rhs.0))
            });
            methods.add_meta_function(MetaMethod::Sub, |lua, args| {
                let hlist_pat![lhs, rhs] = lua.unpack::<HList![MyUserData, MyUserData]>(args)?;
                lua.pack(MyUserData(lhs.0 - rhs.0))
            });
            methods.add_meta_method(MetaMethod::Index, |lua, data, args| {
                let index = lua.unpack::<LuaString>(args)?;
                if index.to_str()? == "inner" {
                    lua.pack(data.0)
                } else {
                    Err("no such custom index".to_lua_err())
                }
            });
        }
    }

    let lua = Lua::new();
    let globals = lua.globals();
    globals.set("userdata1", MyUserData(7)).unwrap();
    globals.set("userdata2", MyUserData(3)).unwrap();
    assert_eq!(
        lua.eval::<MyUserData>("userdata1 + userdata2", None)
            .unwrap()
            .0,
        10
    );
    assert_eq!(
        lua.eval::<MyUserData>("userdata1 - userdata2", None)
            .unwrap()
            .0,
        4
    );
    assert_eq!(lua.eval::<i64>("userdata1:get()", None).unwrap(), 7);
    assert_eq!(lua.eval::<i64>("userdata2.inner", None).unwrap(), 3);
    assert!(lua.eval::<()>("userdata2.nonexist_field", None).is_err());
}

#[test]
fn test_scope() {
    let lua = Lua::new();
    let globals = lua.globals();
    lua.exec::<()>(
        r#"
            touter = {
                tin = {1, 2, 3}
            }
        "#,
        None,
    ).unwrap();

    // Make sure that table gets do not borrow the table, but instead just borrow lua.
    let tin;
    {
        let touter = globals.get::<_, Table>("touter").unwrap();
        tin = touter.get::<_, Table>("tin").unwrap();
    }

    assert_eq!(tin.get::<_, i64>(1).unwrap(), 1);
    assert_eq!(tin.get::<_, i64>(2).unwrap(), 2);
    assert_eq!(tin.get::<_, i64>(3).unwrap(), 3);

    // Should not compile, don't know how to test that
    // struct UserData;
    // impl UserData for UserData {};
    // let userdata_ref;
    // {
    //     let touter = globals.get::<_, Table>("touter").unwrap();
    //     touter.set("userdata", lua.create_userdata(UserData).unwrap()).unwrap();
    //     let userdata = touter.get::<_, AnyUserData>("userdata").unwrap();
    //     userdata_ref = userdata.borrow::<UserData>();
    // }
}

#[test]
fn test_lua_multi() {
    let lua = Lua::new();
    let globals = lua.globals();
    lua.exec::<()>(
        r#"
            function concat(arg1, arg2)
                return arg1 .. arg2
            end

            function mreturn()
                return 1, 2, 3, 4, 5, 6
            end
        "#,
        None,
    ).unwrap();

    let concat = globals.get::<_, Function>("concat").unwrap();
    let mreturn = globals.get::<_, Function>("mreturn").unwrap();

    assert_eq!(
        concat.call::<_, String>(hlist!["foo", "bar"]).unwrap(),
        "foobar"
    );
    let hlist_pat![a, b] = mreturn.call::<_, HList![u64, u64]>(hlist![]).unwrap();
    assert_eq!((a, b), (1, 2));
    let hlist_pat![a, b, Variadic(v)] =
        mreturn.call::<_, HList![u64, u64, Variadic<u64>]>(hlist![]).unwrap();
    assert_eq!((a, b), (1, 2));
    assert_eq!(v, vec![3, 4, 5, 6]);
}

#[test]
fn test_coercion() {
    let lua = Lua::new();
    let globals = lua.globals();
    lua.exec::<()>(
        r#"
            int = 123
            str = "123"
            num = 123.0
        "#,
        None,
    ).unwrap();

    assert_eq!(globals.get::<_, String>("int").unwrap(), "123");
    assert_eq!(globals.get::<_, i32>("str").unwrap(), 123);
    assert_eq!(globals.get::<_, i32>("num").unwrap(), 123);
}

#[test]
fn test_error() {
    #[derive(Debug)]
    pub struct TestError;

    impl fmt::Display for TestError {
        fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
            write!(fmt, "test error")
        }
    }

    impl error::Error for TestError {
        fn description(&self) -> &str {
            "test error"
        }

        fn cause(&self) -> Option<&error::Error> {
            None
        }
    }

    let lua = Lua::new();
    let globals = lua.globals();
    lua.exec::<()>(
        r#"
            function no_error()
            end

            function lua_error()
                error("this is a lua error")
            end

            function rust_error()
                rust_error_function()
            end

            function return_error()
                local status, res = pcall(rust_error_function)
                assert(not status)
                return res
            end

            function return_string_error()
                return "this should be converted to an error"
            end

            function test_pcall()
                local testvar = 0

                pcall(function(arg)
                    testvar = testvar + arg
                    error("should be ignored")
                end, 3)

                local function handler(err)
                    testvar = testvar + err
                    return "should be ignored"
                end

                local status, res = xpcall(function()
                    error(5)
                end, handler)
                assert(not status)

                if testvar ~= 8 then
                    error("testvar had the wrong value, pcall / xpcall misbehaving "..testvar)
                end
            end

            function understand_recursion()
                understand_recursion()
            end
        "#,
        None,
    ).unwrap();

    let rust_error_function = lua.create_function(|_, _| Err(TestError.to_lua_err()));
    globals
        .set("rust_error_function", rust_error_function)
        .unwrap();

    let no_error = globals.get::<_, Function>("no_error").unwrap();
    let lua_error = globals.get::<_, Function>("lua_error").unwrap();
    let rust_error = globals.get::<_, Function>("rust_error").unwrap();
    let return_error = globals.get::<_, Function>("return_error").unwrap();
    let return_string_error = globals
        .get::<_, Function>("return_string_error")
        .unwrap();
    let test_pcall = globals.get::<_, Function>("test_pcall").unwrap();
    let understand_recursion = globals
        .get::<_, Function>("understand_recursion")
        .unwrap();

    assert!(no_error.call::<_, ()>(()).is_ok());
    match lua_error.call::<_, ()>(()) {
        Err(Error::RuntimeError(_)) => {}
        Err(_) => panic!("error is not RuntimeError kind"),
        _ => panic!("error not returned"),
    }
    match rust_error.call::<_, ()>(()) {
        Err(Error::CallbackError(_, _)) => {}
        Err(_) => panic!("error is not CallbackError kind"),
        _ => panic!("error not returned"),
    }

    match return_error.call::<_, Value>(()) {
        Ok(Value::Error(_)) => {}
        _ => panic!("Value::Error not returned"),
    }

    assert!(return_string_error.call::<_, Error>(()).is_ok());

    match lua.eval::<()>("if youre happy and you know it syntax error", None) {
        Err(Error::SyntaxError(_)) => {}
        Err(_) => panic!("error is not LuaSyntaxError::Syntax kind"),
        _ => panic!("error not returned"),
    }
    match lua.eval::<()>("function i_will_finish_what_i()", None) {
        Err(Error::IncompleteStatement(_)) => {}
        Err(_) => panic!("error is not LuaSyntaxError::IncompleteStatement kind"),
        _ => panic!("error not returned"),
    }

    test_pcall.call::<_, ()>(()).unwrap();

    assert!(understand_recursion.call::<_, ()>(()).is_err());

    match catch_unwind(|| -> Result<()> {
        let lua = Lua::new();
        let globals = lua.globals();

        lua.exec::<()>(
            r#"
                function rust_panic()
                    pcall(function () rust_panic_function() end)
                end
            "#,
            None,
        )?;
        let rust_panic_function = lua.create_function(|_, _| {
            panic!("expected panic, this panic should be caught in rust")
        });
        globals.set("rust_panic_function", rust_panic_function)?;

        let rust_panic = globals.get::<_, Function>("rust_panic")?;

        rust_panic.call::<_, ()>(())
    }) {
        Ok(Ok(_)) => panic!("no panic was detected, pcall caught it!"),
        Ok(Err(e)) => panic!("error during panic test {:?}", e),
        Err(_) => {}
    };

    match catch_unwind(|| -> Result<()> {
        let lua = Lua::new();
        let globals = lua.globals();

        lua.exec::<()>(
            r#"
                function rust_panic()
                    xpcall(function() rust_panic_function() end, function() end)
                end
            "#,
            None,
        )?;
        let rust_panic_function = lua.create_function(|_, _| {
            panic!("expected panic, this panic should be caught in rust")
        });
        globals.set("rust_panic_function", rust_panic_function)?;

        let rust_panic = globals.get::<_, Function>("rust_panic")?;

        rust_panic.call::<_, ()>(())
    }) {
        Ok(Ok(_)) => panic!("no panic was detected, xpcall caught it!"),
        Ok(Err(e)) => panic!("error during panic test {:?}", e),
        Err(_) => {}
    };
}

#[test]
fn test_thread() {
    let lua = Lua::new();
    let thread = lua.create_thread(
        lua.eval::<Function>(
            r#"
                function (s)
                    local sum = s
                    for i = 1,4 do
                        sum = sum + coroutine.yield(sum)
                    end
                    return sum
                end
            "#,
            None,
        ).unwrap(),
    );

    assert_eq!(thread.status(), ThreadStatus::Active);
    assert_eq!(thread.resume::<_, i64>(0).unwrap(), 0);
    assert_eq!(thread.status(), ThreadStatus::Active);
    assert_eq!(thread.resume::<_, i64>(1).unwrap(), 1);
    assert_eq!(thread.status(), ThreadStatus::Active);
    assert_eq!(thread.resume::<_, i64>(2).unwrap(), 3);
    assert_eq!(thread.status(), ThreadStatus::Active);
    assert_eq!(thread.resume::<_, i64>(3).unwrap(), 6);
    assert_eq!(thread.status(), ThreadStatus::Active);
    assert_eq!(thread.resume::<_, i64>(4).unwrap(), 10);
    assert_eq!(thread.status(), ThreadStatus::Dead);

    let accumulate = lua.create_thread(
        lua.eval::<Function>(
            r#"
                function (sum)
                    while true do
                        sum = sum + coroutine.yield(sum)
                    end
                end
            "#,
            None,
        ).unwrap(),
    );

    for i in 0..4 {
        accumulate.resume::<_, ()>(i).unwrap();
    }
    assert_eq!(accumulate.resume::<_, i64>(4).unwrap(), 10);
    assert_eq!(accumulate.status(), ThreadStatus::Active);
    assert!(accumulate.resume::<_, ()>("error").is_err());
    assert_eq!(accumulate.status(), ThreadStatus::Error);

    let thread = lua.eval::<Thread>(
        r#"
            coroutine.create(function ()
                while true do
                    coroutine.yield(42)
                end
            end)
        "#,
        None,
    ).unwrap();
    assert_eq!(thread.status(), ThreadStatus::Active);
    assert_eq!(thread.resume::<_, i64>(()).unwrap(), 42);

    let thread: Thread = lua.eval(
        r#"
            coroutine.create(function(arg)
                assert(arg == 42)
                local yieldarg = coroutine.yield(123)
                assert(yieldarg == 43)
                return 987
            end)
        "#,
        None,
    ).unwrap();

    assert_eq!(thread.resume::<_, u32>(42).unwrap(), 123);
    assert_eq!(thread.resume::<_, u32>(43).unwrap(), 987);

    match thread.resume::<_, u32>(()) {
        Err(Error::CoroutineInactive) => {}
        Err(_) => panic!("resuming dead coroutine error is not CoroutineInactive kind"),
        _ => panic!("resuming dead coroutine did not return error"),
    }
}

#[test]
fn test_lightuserdata() {
    let lua = Lua::new();
    let globals = lua.globals();
    lua.exec::<()>(
        r#"
            function id(a)
                return a
            end
        "#,
        None,
    ).unwrap();
    let res = globals
        .get::<_, Function>("id")
        .unwrap()
        .call::<_, LightUserData>(LightUserData(42 as *mut c_void))
        .unwrap();
    assert_eq!(res, LightUserData(42 as *mut c_void));
}

#[test]
fn test_table_error() {
    let lua = Lua::new();
    let globals = lua.globals();
    lua.exec::<()>(
        r#"
            table = {}
            setmetatable(table, {
                __index = function()
                    error("lua error")
                end,
                __newindex = function()
                    error("lua error")
                end,
                __len = function()
                    error("lua error")
                end
            })
        "#,
        None,
    ).unwrap();

    let bad_table: Table = globals.get("table").unwrap();
    assert!(bad_table.set(1, 1).is_err());
    assert!(bad_table.get::<_, i32>(1).is_err());
    assert!(bad_table.len().is_err());
    assert!(bad_table.raw_set(1, 1).is_ok());
    assert!(bad_table.raw_get::<_, i32>(1).is_ok());
    assert_eq!(bad_table.raw_len(), 1);
}

#[test]
fn test_result_conversions() {
    let lua = Lua::new();
    let globals = lua.globals();

    let err = lua.create_function(|lua, _| {
        lua.pack(Err::<String, _>(
            "only through failure can we succeed".to_lua_err(),
        ))
    });
    let ok = lua.create_function(|lua, _| lua.pack(Ok::<_, Error>("!".to_owned())));

    globals.set("err", err).unwrap();
    globals.set("ok", ok).unwrap();

    lua.exec::<()>(
        r#"
            local r, e = err()
            assert(r == nil)
            assert(tostring(e) == "only through failure can we succeed")

            local r, e = ok()
            assert(r == "!")
            assert(e == nil)
        "#,
        None,
    ).unwrap();
}

#[test]
fn test_num_conversion() {
    let lua = Lua::new();
    let globals = lua.globals();

    globals.set("n", "1.0").unwrap();
    assert_eq!(globals.get::<_, i64>("n").unwrap(), 1);
    assert_eq!(globals.get::<_, f64>("n").unwrap(), 1.0);
    assert_eq!(globals.get::<_, String>("n").unwrap(), "1.0");

    globals.set("n", "1.5").unwrap();
    assert!(globals.get::<_, i64>("n").is_err());
    assert_eq!(globals.get::<_, f64>("n").unwrap(), 1.5);
    assert_eq!(globals.get::<_, String>("n").unwrap(), "1.5");

    globals.set("n", 1.5).unwrap();
    assert!(globals.get::<_, i64>("n").is_err());
    assert_eq!(globals.get::<_, f64>("n").unwrap(), 1.5);
    assert_eq!(globals.get::<_, String>("n").unwrap(), "1.5");

    lua.exec::<()>("a = math.huge", None).unwrap();
    assert!(globals.get::<_, i64>("n").is_err());
}

#[test]
#[should_panic]
fn test_expired_userdata() {
    struct Userdata {
        id: u8,
    }

    impl LuaUserDataType for Userdata {
        fn add_methods(methods: &mut LuaUserDataMethods<Self>) {
            methods.add_method("access", |lua, this, _| {
                assert!(this.id == 123);
                lua.pack(())
            });
        }
    }

    let lua = Lua::new();
    {
        let globals = lua.globals();
        globals.set("userdata", Userdata { id: 123 }).unwrap();
    }

    lua.eval::<()>(r#"
        local tbl = setmetatable({
            userdata = userdata
        }, { __gc = function(self)
            -- resurrect userdata
            hatch = self.userdata
        end })

        tbl = nil
        userdata = nil  -- make table and userdata collectable
        collectgarbage("collect")
        hatch:access()
    "#, None).unwrap();
}

#[test]
fn detroys_userdata() {
    use std::sync::atomic::{Ordering, AtomicBool, ATOMIC_BOOL_INIT};

    static DROPPED: AtomicBool = ATOMIC_BOOL_INIT;

    struct Userdata;

    impl LuaUserDataType for Userdata {}

    impl Drop for Userdata {
        fn drop(&mut self) {
            DROPPED.store(true, Ordering::SeqCst);
        }
    }

    let lua = Lua::new();
    {
        let globals = lua.globals();
        globals.set("userdata", Userdata).unwrap();
    }

    assert_eq!(DROPPED.load(Ordering::SeqCst), false);
    drop(lua);  // should destroy all objects
    assert_eq!(DROPPED.load(Ordering::SeqCst), true);
}
