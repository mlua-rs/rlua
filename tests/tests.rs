use bstr::BString;
use std::iter::FromIterator;
use std::panic::catch_unwind;
use std::sync::Arc;
use std::{error, f32, f64, fmt};

use rlua::{
    Error, ExternalError, Function, LuaOptions, Lua, Nil, Result, StdLib, String, Table, UserData,
    Value, Variadic,
    RluaCompat
};

#[test]
fn test_load() {
    Lua::new().context(|lua| {
        let func = lua.load("return 1+2").into_function().unwrap();
        let result: i32 = func.call(()).unwrap();
        assert_eq!(result, 3);

        assert!(lua.load("§$%§&$%&").exec().is_err());
    });
}

#[test]
fn test_load_empty() {
    Lua::new().context(|lua| {
        assert!(lua.load(b"".as_slice()).exec().is_ok());
    });
}

#[test]
fn test_debug() {
    let lua = unsafe { Lua::unsafe_new() };
    lua.context(|lua| {
        match lua.load("debug").eval().unwrap() {
            Value::Table(_) => {}
            val => panic!("Expected table for debug library, got {:#?}", val),
        }
        let traceback_output = lua.load("debug.traceback()").eval::<String>().unwrap();
        assert_eq!(
            traceback_output.to_str().unwrap().split("\n").next(),
            "stack traceback:".into()
        );
    });
}

#[test]
#[should_panic]
fn test_new_with_debug_panic() {
    let _lua = Lua::new_with(StdLib::DEBUG, LuaOptions::default()).unwrap();
}

#[test]
fn test_exec() {
    Lua::new().context(|lua| {
        let globals = lua.globals();
        lua.load(
            r#"
                res = 'foo'..'bar'
            "#,
        )
        .exec()
        .unwrap();
        assert_eq!(globals.get::<_, String>("res").unwrap(), "foobar");

        let module: Table = lua
            .load(
                r#"
                    local module = {}

                    function module.func()
                        return "hello"
                    end

                    return module
                "#,
            )
            .eval()
            .unwrap();
        assert!(module.contains_key("func").unwrap());
        assert_eq!(
            module
                .get::<_, Function>("func")
                .unwrap()
                .call::<_, String>(())
                .unwrap(),
            "hello"
        );
    });
}

#[test]
fn test_eval() {
    Lua::new().context(|lua| {
        assert_eq!(lua.load("1 + 1").eval::<i32>().unwrap(), 2);
        assert_eq!(lua.load("false == false").eval::<bool>().unwrap(), true);
        assert_eq!(lua.load("return 1 + 2").eval::<i32>().unwrap(), 3);
        match lua.load("if true then").eval::<()>() {
            Err(Error::SyntaxError {
                incomplete_input: true,
                ..
            }) => {}
            r => panic!(
                "expected SyntaxError with incomplete_input=true, got {:?}",
                r
            ),
        }
    });
}

#[test]
fn test_lua_multi() {
    Lua::new().context(|lua| {
        let globals = lua.globals();
        lua.load(
            r#"
                function concat(arg1, arg2)
                    return arg1 .. arg2
                end

                function mreturn()
                    return 1, 2, 3, 4, 5, 6
                end
            "#,
        )
        .exec()
        .unwrap();

        let concat = globals.get::<_, Function>("concat").unwrap();
        let mreturn = globals.get::<_, Function>("mreturn").unwrap();

        assert_eq!(concat.call::<_, String>(("foo", "bar")).unwrap(), "foobar");
        let (a, b) = mreturn.call::<_, (u64, u64)>(()).unwrap();
        assert_eq!((a, b), (1, 2));
        let (a, b, v) = mreturn.call::<_, (u64, u64, Variadic<u64>)>(()).unwrap();
        assert_eq!((a, b), (1, 2));
        assert_eq!(v[..], [3, 4, 5, 6]);
    });
}

#[test]
fn test_coercion() {
    Lua::new().context(|lua| {
        let globals = lua.globals();
        lua.load(
            r#"
                int = 123
                str = "123"
                num = 123.0
            "#,
        )
        .exec()
        .unwrap();

        assert_eq!(globals.get::<_, String>("int").unwrap(), "123");
        assert_eq!(globals.get::<_, i32>("str").unwrap(), 123);
        assert_eq!(globals.get::<_, i32>("num").unwrap(), 123);
    });
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

        fn cause(&self) -> Option<&dyn error::Error> {
            None
        }
    }

    Lua::new().context(|lua| {
        let globals = lua.globals();
        lua.load(
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
                        error(5, 0)
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
        )
        .exec()
        .unwrap();

        let rust_error_function = lua
            .create_function(|_, ()| -> Result<()> { Err(TestError.into_lua_err()) })
            .unwrap();
        globals
            .set("rust_error_function", rust_error_function)
            .unwrap();

        let no_error = globals.get::<_, Function>("no_error").unwrap();
        let lua_error = globals.get::<_, Function>("lua_error").unwrap();
        let rust_error = globals.get::<_, Function>("rust_error").unwrap();
        let return_error = globals.get::<_, Function>("return_error").unwrap();
        let return_string_error = globals.get::<_, Function>("return_string_error").unwrap();
        let test_pcall = globals.get::<_, Function>("test_pcall").unwrap();
        let understand_recursion = globals.get::<_, Function>("understand_recursion").unwrap();

        assert!(no_error.call::<_, ()>(()).is_ok());
        match lua_error.call::<_, ()>(()) {
            Err(Error::RuntimeError(_)) => {}
            Err(_) => panic!("error is not RuntimeError kind"),
            _ => panic!("error not returned"),
        }
        match rust_error.call::<_, ()>(()) {
            Err(Error::CallbackError { .. }) => {}
            Err(_) => panic!("error is not CallbackError kind"),
            _ => panic!("error not returned"),
        }

        match return_error.call::<_, Value>(()) {
            Ok(Value::Error(_)) => {}
            e => panic!("Value::Error not returned: got {:?}", e),
        }

        assert!(return_string_error.call::<_, Error>(()).is_ok());

        match lua
            .load("if youre happy and you know it syntax error")
            .exec()
        {
            Err(Error::SyntaxError {
                incomplete_input: false,
                ..
            }) => {}
            Err(_) => panic!("error is not LuaSyntaxError::Syntax kind"),
            _ => panic!("error not returned"),
        }
        match lua.load("function i_will_finish_what_i()").exec() {
            Err(Error::SyntaxError {
                incomplete_input: true,
                ..
            }) => {}
            Err(_) => panic!("error is not LuaSyntaxError::IncompleteStatement kind"),
            _ => panic!("error not returned"),
        }

        test_pcall.call::<_, ()>(()).unwrap();

        assert!(understand_recursion.call::<_, ()>(()).is_err());
    });

    match catch_unwind(|| -> Result<usize> {
        Lua::new_with(StdLib::ALL_SAFE, LuaOptions::default().catch_rust_panics(false)).unwrap().context(|lua| {
            let globals = lua.globals();

            lua.load(
                r#"
                    function rust_panic()
                        ok, err = pcall(function () rust_panic_function() end)
                        print(tostring(ok), tostring(err))
                        return 17
                    end
                "#,
            )
            .exec()?;
            let rust_panic_function = lua
                .create_function(|_, ()| -> Result<()> { panic!("test_panic") })
                .unwrap();
            globals.set("rust_panic_function", rust_panic_function)?;

            let rust_panic = globals.get::<_, Function>("rust_panic")?;

            dbg!(rust_panic.call::<_, usize>(()))
        })
    }) {
        Ok(Ok(_)) => panic!("no panic was detected, pcall caught it!"),
        Ok(Err(e)) => panic!("error during panic test {:?}", e),
        Err(p) => assert!(*p.downcast::<&str>().unwrap() == "test_panic"),
    };

    match catch_unwind(|| -> Result<()> {
        Lua::new_with(StdLib::ALL_SAFE, LuaOptions::default().catch_rust_panics(false)).unwrap().context(|lua| {
            let globals = lua.globals();

            lua.load(
                r#"
                    function rust_panic()
                        xpcall(function() rust_panic_function() end, function() end)
                    end
                "#,
            )
            .exec()?;
            let rust_panic_function = lua
                .create_function(|_, ()| -> Result<()> { panic!("test_panic") })
                .unwrap();
            globals.set("rust_panic_function", rust_panic_function)?;

            let rust_panic = globals.get::<_, Function>("rust_panic")?;

            rust_panic.call::<_, ()>(())
        })
    }) {
        Ok(Ok(_)) => panic!("no panic was detected, xpcall caught it!"),
        Ok(Err(e)) => panic!("error during panic test {:?}", e),
        Err(p) => assert!(*p.downcast::<&str>().unwrap() == "test_panic"),
    };
}

// Test that skipping the pcall/xpcall wrappers works.
// TODO: The only way to test that the behaviour is correct is to
// cause it to abort().
/*
#[test]
fn test_error_nopcall_wrap() {
    match catch_unwind(|| -> Result<()> {
        unsafe {
            Lua::unsafe_new_with_flags(StdLib::ALL, InitFlags::DEFAULT - InitFlags::PCALL_WRAPPERS).context(|lua| {
                let globals = lua.globals();

                lua.load(
                    r#"
                        function rust_panic()
                            pcall(function () rust_panic_function() end)
                        end
                    "#,
                )
                .exec()?;
                let rust_panic_function = lua
                    .create_function(|_, ()| -> Result<()> { panic!("test_panic") })
                    .unwrap();
                globals.set("rust_panic_function", rust_panic_function)?;

                let rust_panic = globals.get::<_, Function>("rust_panic")?;

                rust_panic.call::<_, ()>(())
            })
        }
    }) {
        Ok(Ok(_)) => panic!("no panic was detected, pcall caught it!"),
        Ok(Err(e)) => panic!("error during panic test {:?}", e),
        Err(p) => assert!(*p.downcast::<&str>().unwrap() == "test_panic"),
    };

    match catch_unwind(|| -> Result<()> {
        unsafe {
            Lua::unsafe_new_with_flags(StdLib::ALL, InitFlags::DEFAULT - InitFlags::PCALL_WRAPPERS).context(|lua| {
                let globals = lua.globals();

                lua.load(
                    r#"
                        function rust_panic()
                            xpcall(function() rust_panic_function() end, function() end)
                        end
                    "#,
                )
                .exec()?;
                let rust_panic_function = lua
                    .create_function(|_, ()| -> Result<()> { panic!("test_panic") })
                    .unwrap();
                globals.set("rust_panic_function", rust_panic_function)?;

                let rust_panic = globals.get::<_, Function>("rust_panic")?;

                rust_panic.call::<_, ()>(())
            })
        }
    }) {
        Ok(Ok(_)) => panic!("no panic was detected, xpcall caught it!"),
        Ok(Err(e)) => panic!("error during panic test {:?}", e),
        Err(p) => assert!(*p.downcast::<&str>().unwrap() == "test_panic"),
    };
}
*/

// The following tests are no longer relevant now that `rlua` is backed by `mlua`
// which takes slightly different decisions on loading unsafe code; it is up to
// the user to prevent loading of Lua bytecode if required.
/*
#[test]
fn test_load_wrappers() {
    Lua::new().context(|lua| {
        let globals = lua.globals();
        lua.load(
            r#"
                x = 0
                function incx()
                    x = x + 1
                end
                binchunk = string.dump(incx)
            "#,
        )
        .exec()
        .unwrap();
        assert_eq!(globals.get::<_, u32>("x").unwrap(), 0);

        lua.load(
            r#"
                incx()
            "#,
        )
        .exec()
        .unwrap();
        assert_eq!(globals.get::<_, u32>("x").unwrap(), 1);
        lua.load(
            r#"
                assert(type(binchunk) == "string")
                local binchunk_copy = binchunk
                chunk = load(function ()
                    local result = binchunk_copy
                    binchunk_copy = nil
                    return result
                end, "bad", "bt")
                print(chunk)
                assert(chunk == nil)
            "#,
        )
        .exec()
        .unwrap();
        assert_eq!(globals.get::<_, u32>("x").unwrap(), 1);
        lua.load(
            r#"
                local s = "x = x + 4"
                local function loader()
                    local result = s
                    s = nil
                    return result
                end
                chunk = load(loader)
                assert(chunk ~= nil)
                chunk()
            "#,
        )
        .exec()
        .unwrap();
        assert_eq!(globals.get::<_, u32>("x").unwrap(), 5);
    });
}
*/

#[test]
fn test_no_load_wrappers() {
    unsafe {
        Lua::unsafe_new_with(
            StdLib::ALL,
            LuaOptions::default()
        )
        .context(|lua| {
            let globals = lua.globals();
            lua.load(
                r#"
                x = 0
                function incx()
                    x = x + 1
                end
                binchunk = string.dump(incx)
            "#,
            )
            .exec()
            .unwrap();
            assert_eq!(globals.get::<_, u32>("x").unwrap(), 0);

            lua.load(
                r#"
                incx()
            "#,
            )
            .exec()
            .unwrap();
            assert_eq!(globals.get::<_, u32>("x").unwrap(), 1);
            lua.load(
                r#"
                assert(type(binchunk) == "string")
                assert(binchunk:byte(1) == 27)
                local stringsource = binchunk
                local loader = function ()
                    local result = stringsource
                    stringsource = nil
                    return result
                end
                if _VERSION ~= "Lua 5.1" then
                    -- Lua 5.1 doesn't support the mode parameter.
                    chunk = load(binchunk, "fail", "t")
                    assert(chunk == nil)
                end
                chunk = load(loader, "good", "bt")
                assert(chunk ~= nil)
                chunk()
                stringsource = "x = x + 3"
                chunk = load(loader)
                chunk()
            "#,
            )
            .exec()
            .unwrap();
            assert_eq!(globals.get::<_, u32>("x").unwrap(), 5);
        })
    };
}

// The following tests are no longer relevant now that `rlua` is backed by `mlua`
// which takes slightly different decisions on loading unsafe code.
/*
#[test]
fn test_loadfile_wrappers() {
    let mut tmppath = std::env::temp_dir();
    tmppath.push("test_loadfile_wrappers.lua");

    Lua::new().context(|lua| {
        let globals = lua.globals();
        globals.set("filename", tmppath.to_str().unwrap()).unwrap();
        lua.load(
            r#"
                x = 0
                function incx()
                    x = x + 1
                end
                binchunk = string.dump(incx)
            "#,
        )
        .exec()
        .unwrap();
        assert_eq!(globals.get::<_, u32>("x").unwrap(), 0);
        let binchunk = globals.get::<_, BString>("binchunk").unwrap();

        lua.load(
            r#"
                incx()
            "#,
        )
        .exec()
        .unwrap();
        assert_eq!(globals.get::<_, u32>("x").unwrap(), 1);
        std::fs::write(&tmppath, binchunk).unwrap();
        lua.load(
            r#"
                chunk = loadfile(filename)
                assert(chunk == nil)
            "#,
        )
        .exec()
        .unwrap();
        assert_eq!(globals.get::<_, u32>("x").unwrap(), 1);
        std::fs::write(&tmppath, "x = x + 4").unwrap();
        lua.load(
            r#"
                chunk = loadfile(filename)
                assert(chunk ~= nil)
                chunk()
            "#,
        )
        .exec()
        .unwrap();
        assert_eq!(globals.get::<_, u32>("x").unwrap(), 5);
    });
}
*/

#[test]
fn test_no_loadfile_wrappers() {
    let mut tmppath = std::env::temp_dir();
    let mut tmppath2 = tmppath.clone();
    tmppath.push("test_no_loadfile_wrappers.lua");
    tmppath2.push("test_no_loadfile_wrappers2.lua");

    unsafe {
        Lua::unsafe_new_with(
            StdLib::ALL,
            LuaOptions::default()
        )
        .context(|lua| {
            let globals = lua.globals();
            globals.set("filename", tmppath.to_str().unwrap()).unwrap();
            globals
                .set("filename2", tmppath2.to_str().unwrap())
                .unwrap();
            lua.load(
                r#"
                x = 0
                function incx()
                    x = x + 1
                end
                binchunk = string.dump(incx)
            "#,
            )
            .exec()
            .unwrap();
            assert_eq!(globals.get::<_, u32>("x").unwrap(), 0);
            let binchunk = globals.get::<_, BString>("binchunk").unwrap();

            lua.load(
                r#"
                incx()
            "#,
            )
            .exec()
            .unwrap();
            assert_eq!(globals.get::<_, u32>("x").unwrap(), 1);
            std::fs::write(&tmppath, binchunk).unwrap();
            std::fs::write(&tmppath2, "x = x + 3").unwrap();
            lua.load(
                r#"
                if _VERSION ~= "Lua 5.1" then
                    -- Lua 5.1 doesn't have the mode argument, so is
                    -- effectively always "bt".
                    chunk = loadfile(filename, "t")
                    assert(chunk == nil)
                end
                chunk = loadfile(filename, "bt")
                assert(chunk ~= nil)
                chunk()
                chunk = loadfile(filename2)
                chunk()
            "#,
            )
            .exec()
            .unwrap();
            assert_eq!(globals.get::<_, u32>("x").unwrap(), 5);
        })
    };
}

// The following tests are no longer relevant now that `rlua` is backed by `mlua`
// which takes slightly different decisions on loading unsafe code; it is up to
// the user to prevent loading of Lua bytecode if required.
/*
#[test]
fn test_dofile_wrappers() {
    let mut tmppath = std::env::temp_dir();
    tmppath.push("test_dofile_wrappers.lua");

    Lua::new().context(|lua| {
        let globals = lua.globals();
        globals.set("filename", tmppath.to_str().unwrap()).unwrap();
        lua.load(
            r#"
                x = 0
                function incx()
                    x = x + 1
                end
                binchunk = string.dump(incx)
            "#,
        )
        .exec()
        .unwrap();
        assert_eq!(globals.get::<_, u32>("x").unwrap(), 0);
        let binchunk = globals.get::<_, BString>("binchunk").unwrap();

        lua.load(
            r#"
                incx()
            "#,
        )
        .exec()
        .unwrap();
        assert_eq!(globals.get::<_, u32>("x").unwrap(), 1);
        std::fs::write(&tmppath, binchunk).unwrap();
        lua.load(
            r#"
                ok, err = pcall(dofile, filename)
                assert(not ok)
                assert(err:match("rlua dofile: attempt to load bytecode"))
            "#,
        )
        .exec()
        .unwrap();
        assert_eq!(globals.get::<_, u32>("x").unwrap(), 1);
        std::fs::write(&tmppath, "x = x + 4").unwrap();
        lua.load(
            r#"
                ok, ret = pcall(dofile, filename)
                assert(ok)
            "#,
        )
        .exec()
        .unwrap();
        assert_eq!(globals.get::<_, u32>("x").unwrap(), 5);
    });
}
*/

#[test]
fn test_no_dofile_wrappers() {
    let mut tmppath = std::env::temp_dir();
    tmppath.push("test_no_dofile_wrappers.lua");

    unsafe {
        Lua::unsafe_new_with(
            StdLib::ALL,
            LuaOptions::default()
        )
        .context(|lua| {
            let globals = lua.globals();
            globals.set("filename", tmppath.to_str().unwrap()).unwrap();
            lua.load(
                r#"
                    x = 0
                    function incx()
                        x = x + 1
                    end
                    binchunk = string.dump(incx)
                "#,
            )
            .exec()
            .unwrap();
            assert_eq!(globals.get::<_, u32>("x").unwrap(), 0);
            let binchunk = globals.get::<_, BString>("binchunk").unwrap();

            lua.load(
                r#"
                    incx()
                "#,
            )
            .exec()
            .unwrap();
            assert_eq!(globals.get::<_, u32>("x").unwrap(), 1);
            std::fs::write(&tmppath, binchunk).unwrap();
            lua.load(
                r#"
                    ok, ret = pcall(dofile, filename)
                    assert(ok)
                "#,
            )
            .exec()
            .unwrap();
            assert_eq!(globals.get::<_, u32>("x").unwrap(), 2);
            std::fs::write(&tmppath, "x = x + 4").unwrap();
            lua.load(
                r#"
                    ok, ret = pcall(dofile, filename)
                    assert(ok)
                "#,
            )
            .exec()
            .unwrap();
            assert_eq!(globals.get::<_, u32>("x").unwrap(), 6);
        });
    }
}

#[test]
fn test_loadstring_wrappers() {
    Lua::new().context(|lua| {
        let globals = lua.globals();
        if globals.get::<_, Function>("loadstring").is_err() {
            // Loadstring is not present in Lua 5.4, and only with a
            // compatibility mode in Lua 5.3.
            return;
        }
        lua.load(
            r#"
                x = 0
                function incx()
                    x = x + 1
                end
                binchunk = string.dump(incx)
            "#,
        )
        .exec()
        .unwrap();
        assert_eq!(globals.get::<_, u32>("x").unwrap(), 0);

        lua.load(
            r#"
                incx()
            "#,
        )
        .exec()
        .unwrap();
        assert_eq!(globals.get::<_, u32>("x").unwrap(), 1);
        lua.load(
            r#"
                assert(type(binchunk) == "string")
                chunk = loadstring(binchunk)
                assert(chunk == nil)
            "#,
        )
        .exec()
        .unwrap();
        assert_eq!(globals.get::<_, u32>("x").unwrap(), 1);
        lua.load(
            r#"
                local s = "x = x + 4"
                chunk = loadstring(s)
                assert(chunk ~= nil)
                chunk()
            "#,
        )
        .exec()
        .unwrap();
        assert_eq!(globals.get::<_, u32>("x").unwrap(), 5);
    });
}

#[test]
fn test_no_loadstring_wrappers() {
    unsafe {
        Lua::unsafe_new_with(
            StdLib::ALL,
            LuaOptions::default()
        )
        .context(|lua| {
            let globals = lua.globals();
            if globals.get::<_, Function>("loadstring").is_err() {
                // Loadstring is not present in Lua 5.4, and only with a
                // compatibility mode in Lua 5.3.
                return;
            }
            lua.load(
                r#"
                x = 0
                function incx()
                    x = x + 1
                end
                binchunk = string.dump(incx)
            "#,
            )
            .exec()
            .unwrap();
            assert_eq!(globals.get::<_, u32>("x").unwrap(), 0);

            lua.load(
                r#"
                incx()
            "#,
            )
            .exec()
            .unwrap();
            assert_eq!(globals.get::<_, u32>("x").unwrap(), 1);
            lua.load(
                r#"
                assert(type(binchunk) == "string")
                assert(binchunk:byte(1) == 27)
                chunk = loadstring(binchunk)
                assert(chunk ~= nil)
                chunk()
                chunk = loadstring("x = x + 3")
                chunk()
            "#,
            )
            .exec()
            .unwrap();
            assert_eq!(globals.get::<_, u32>("x").unwrap(), 5);
        })
    };
}

// The following tests are no longer relevant now that `rlua` is backed by `mlua`
// which takes slightly different decisions on loading unsafe code.
/*
#[test]
fn test_default_loadlib() {
    Lua::new().context(|lua| {
        let globals = lua.globals();
        let package = globals.get::<_, Table>("package").unwrap();
        let loadlib = package.get::<_, Function>("loadlib");
        assert!(loadlib.is_err());

        lua.load(
            r#"
            assert(#(package.loaders or package.searchers) == 2)
            "#,
        )
        .exec()
        .unwrap();
    });
}
*/

#[test]
fn test_no_remove_loadlib() {
    unsafe {
        Lua::unsafe_new_with(
            StdLib::ALL,
            LuaOptions::default()
        )
        .context(|lua| {
            let globals = lua.globals();
            let package = globals.get::<_, Table>("package").unwrap();
            let _loadlib = package.get::<_, Function>("loadlib").unwrap();

            lua.load(
                r#"
                assert(#(package.loaders or package.searchers) == 4)
                "#,
            )
            .exec()
            .unwrap();
        });
    }
}

#[test]
fn test_result_conversions() {
    Lua::new().context(|lua| {
        let globals = lua.globals();

        let err = lua
            .create_function(|_, ()| {
                Ok(Err::<String, _>(
                    "only through failure can we succeed".into_lua_err(),
                ))
            })
            .unwrap();
        let ok = lua
            .create_function(|_, ()| Ok(Ok::<_, Error>("!".to_owned())))
            .unwrap();

        globals.set("err", err).unwrap();
        globals.set("ok", ok).unwrap();

        lua.load(
            r#"
                local r, e = err()
                assert(r == nil)
                assert(tostring(e):find("only through failure can we succeed") ~= nil)

                local r, e = ok()
                assert(r == "!")
                assert(e == nil)
            "#,
        )
        .exec()
        .unwrap();
    });
}

#[test]
fn test_num_conversion() {
    Lua::new().context(|lua| {
        assert_eq!(
            lua.coerce_integer(Value::String(lua.create_string("1").unwrap()))
                .unwrap(),
            Some(1)
        );
        assert_eq!(
            lua.coerce_integer(Value::String(lua.create_string("1.0").unwrap()))
                .unwrap(),
            Some(1)
        );
        assert_eq!(
            lua.coerce_integer(Value::String(lua.create_string("1.5").unwrap()))
                .unwrap(),
            None
        );

        assert_eq!(
            lua.coerce_number(Value::String(lua.create_string("1").unwrap()))
                .unwrap(),
            Some(1.0)
        );
        assert_eq!(
            lua.coerce_number(Value::String(lua.create_string("1.0").unwrap()))
                .unwrap(),
            Some(1.0)
        );
        assert_eq!(
            lua.coerce_number(Value::String(lua.create_string("1.5").unwrap()))
                .unwrap(),
            Some(1.5)
        );

        assert_eq!(lua.load("1.0").eval::<i64>().unwrap(), 1);
        assert_eq!(lua.load("1.0").eval::<f64>().unwrap(), 1.0);
        #[cfg(not(rlua_lua51))]
        assert_eq!(
            lua.load("1.0").eval::<String>().unwrap().to_str().unwrap(),
            "1.0"
        );
        #[cfg(rlua_lua51)]
        assert_eq!(
            lua.load("1.0").eval::<String>().unwrap().to_str().unwrap(),
            "1"
        );

        assert_eq!(lua.load("1.5").eval::<i64>().unwrap(), 1);
        assert_eq!(lua.load("1.5").eval::<f64>().unwrap(), 1.5);
        assert_eq!(lua.load("1.5").eval::<String>().unwrap(), "1.5");

        assert!(lua.load("-1").eval::<u64>().is_err());
        assert_eq!(lua.load("-1").eval::<i64>().unwrap(), -1);

        assert!(lua.unpack::<u64>(lua.pack(1u128 << 64).unwrap()).is_err());
        assert!(lua.load("math.huge").eval::<i64>().is_err());

        assert_eq!(
            lua.unpack::<f64>(lua.pack(f32::MAX).unwrap()).unwrap(),
            f32::MAX as f64
        );
        assert_eq!(lua.unpack::<f32>(lua.pack(f64::MAX).unwrap()).unwrap(),
                   f32::INFINITY);

        assert_eq!(
            lua.unpack::<i128>(lua.pack(1i128 << 64).unwrap()).unwrap(),
            1i128 << 64
        );
    });
}

#[test]
fn test_pcall_xpcall() {
    Lua::new().context(|lua| {
        let globals = lua.globals();

        // make sure that we handle not enough arguments

        assert!(lua.load("pcall()").exec().is_err());
        assert!(lua.load("xpcall()").exec().is_err());
        assert!(lua.load("xpcall(function() end)").exec().is_err());

        // Make sure that the return values from are correct on success

        let (r, e) = lua
            .load("pcall(function(p) return p end, 'foo')")
            .eval::<(bool, String)>()
            .unwrap();
        assert!(r);
        assert_eq!(e, "foo");

        let (r, e) = lua
            .load("xpcall(function(p) return p end, print, 'foo')")
            .eval::<(bool, String)>()
            .unwrap();
        assert!(r);
        assert_eq!(e, "foo");

        // Make sure that the return values are correct on errors, and that error handling works

        lua.load(
            r#"
                pcall_error = nil
                pcall_status, pcall_error = pcall(error, "testerror")

                xpcall_error = nil
                xpcall_status, _ = xpcall(error, function(err) xpcall_error = err end, "testerror")
            "#,
        )
        .exec()
        .unwrap();

        assert_eq!(globals.get::<_, bool>("pcall_status").unwrap(), false);
        assert_eq!(
            globals.get::<_, String>("pcall_error").unwrap(),
            "testerror"
        );

        assert_eq!(globals.get::<_, bool>("xpcall_statusr").unwrap(), false);
        assert_eq!(
            globals.get::<_, String>("xpcall_error").unwrap(),
            "testerror"
        );

        // Make sure that weird xpcall error recursion at least doesn't cause unsafety or panics.
        lua.load(
            r#"
                function xpcall_recursion()
                    xpcall(error, function(err) error(err) end, "testerror")
                end
            "#,
        )
        .exec()
        .unwrap();
        let _ = globals
            .get::<_, Function>("xpcall_recursion")
            .unwrap()
            .call::<_, ()>(());
    });
}

#[test]
fn test_recursive_mut_callback_error() {
    Lua::new().context(|lua| {
        let mut v = Some(Box::new(123));
        let f = lua
            .create_function_mut::<_, (), _>(move |lua, mutate: bool| {
                if mutate {
                    v = None;
                } else {
                    // Produce a mutable reference
                    let r = v.as_mut().unwrap();
                    // Whoops, this will recurse into the function and produce another mutable reference!
                    lua.globals().get::<_, Function>("f")?.call::<_, ()>(true)?;
                    println!("Should not get here, mutable aliasing has occurred!");
                    println!("value at {:p}", r as *mut _);
                    println!("value is {}", r);
                }

                Ok(())
            })
            .unwrap();
        lua.globals().set("f", f).unwrap();
        match lua
            .globals()
            .get::<_, Function>("f")
            .unwrap()
            .call::<_, ()>(false)
        {
            Err(Error::CallbackError { ref cause, .. }) => match *cause.as_ref() {
                Error::CallbackError { ref cause, .. } => match *cause.as_ref() {
                    Error::RecursiveMutCallback { .. } => {}
                    ref other => panic!("incorrect result: {:?}", other),
                },
                ref other => panic!("incorrect result: {:?}", other),
            },
            other => panic!("incorrect result: {:?}", other),
        };
    });
}

#[test]
fn test_set_metatable_nil() {
    Lua::new().context(|lua| {
        lua.load(
            r#"
                a = {}
                setmetatable(a, nil)
            "#,
        )
        .exec()
        .unwrap();
    });
}

#[test]
fn test_named_registry_value() {
    Lua::new().context(|lua| {
        lua.set_named_registry_value::<i32>("test", 42).unwrap();
        let f = lua
            .create_function(move |lua, ()| {
                assert_eq!(lua.named_registry_value::<i32>("test")?, 42);
                Ok(())
            })
            .unwrap();

        f.call::<_, ()>(()).unwrap();

        lua.unset_named_registry_value("test").unwrap();
        match lua.named_registry_value("test").unwrap() {
            Nil => {}
            val => panic!("registry value was not Nil, was {:?}", val),
        };
    });
}

#[test]
fn test_registry_value() {
    Lua::new().context(|lua| {
        let mut r = Some(lua.create_registry_value::<i32>(42).unwrap());
        let f = lua
            .create_function_mut(move |lua, ()| {
                if let Some(r) = r.take() {
                    assert_eq!(lua.registry_value::<i32>(&r)?, 42);
                    lua.remove_registry_value(r).unwrap();
                } else {
                    panic!();
                }
                Ok(())
            })
            .unwrap();

        f.call::<_, ()>(()).unwrap();
    });
}

#[test]
fn test_drop_registry_value() {
    struct MyUserdata(Arc<()>);

    impl UserData for MyUserdata {}

    Lua::new().context(|lua| {
        let rc = Arc::new(());

        let r = lua.create_registry_value(MyUserdata(rc.clone())).unwrap();
        assert_eq!(Arc::strong_count(&rc), 2);

        drop(r);
        lua.expire_registry_values();

        lua.load(r#"collectgarbage("collect")"#).exec().unwrap();

        assert_eq!(Arc::strong_count(&rc), 1);
    });
}

#[test]
fn test_lua_registry_ownership() {
    Lua::new().context(|lua1| {
        Lua::new().context(|lua2| {
            let r1 = lua1.create_registry_value("hello").unwrap();
            let r2 = lua2.create_registry_value("hello").unwrap();

            assert!(lua1.owns_registry_value(&r1));
            assert!(!lua2.owns_registry_value(&r1));
            assert!(lua2.owns_registry_value(&r2));
            assert!(!lua1.owns_registry_value(&r2));
        });
    });
}

#[test]
fn test_mismatched_registry_key() {
    Lua::new().context(|lua1| {
        Lua::new().context(|lua2| {
            let r = lua1.create_registry_value("hello").unwrap();
            match lua2.remove_registry_value(r) {
                Err(Error::MismatchedRegistryKey) => {}
                r => panic!("wrong result type for mismatched registry key, {:?}", r),
            };
        });
    });
}

#[test]
fn too_many_returns() {
    Lua::new().context(|lua| {
        let f = lua
            .create_function(|_, ()| Ok(Variadic::from_iter(1..1000000)))
            .unwrap();
        assert!(f.call::<_, Vec<u32>>(()).is_err());
    });
}

#[test]
fn too_many_arguments() {
    Lua::new().context(|lua| {
        lua.load("function test(...) end").exec().unwrap();
        let args = Variadic::from_iter(1..1000000);
        assert!(lua
            .globals()
            .get::<_, Function>("test")
            .unwrap()
            .call::<_, ()>(args)
            .is_err());
    });
}

#[cfg(not(rlua_luajit))] // LuaJIT doesn't prevent stack overflows
#[test]
fn too_many_recursions() {
    Lua::new().context(|lua| {
        let f = lua
            .create_function(move |lua, ()| {
                lua.globals().get::<_, Function>("f")?.call::<_, ()>(())
            })
            .unwrap();
        lua.globals().set("f", f).unwrap();

        assert!(lua
            .globals()
            .get::<_, Function>("f")
            .unwrap()
            .call::<_, ()>(())
            .is_err());
    });
}

#[test]
fn too_many_binds() {
    Lua::new().context(|lua| {
        let globals = lua.globals();
        lua.load(
            r#"
                function f(...)
                end
            "#,
        )
        .exec()
        .unwrap();

        let concat = globals.get::<_, Function>("f").unwrap();
        assert!(concat.bind(Variadic::from_iter(1..1000000)).is_err());
        assert!(concat
            .call::<_, ()>(Variadic::from_iter(1..1000000))
            .is_err());
    });
}

#[test]
fn large_args() {
    Lua::new().context(|lua| {
        let globals = lua.globals();

        globals
            .set(
                "c",
                lua.create_function(|_, args: Variadic<usize>| {
                    let mut s = 0;
                    for i in 0..args.len() {
                        s += i;
                        assert_eq!(i, args[i]);
                    }
                    Ok(s)
                })
                .unwrap(),
            )
            .unwrap();

        let f: Function = lua
            .load(
                r#"
                    return function(...)
                        return c(...)
                    end
                "#,
            )
            .eval()
            .unwrap();

        assert_eq!(
            f.call::<_, usize>((0..100).collect::<Variadic<usize>>())
                .unwrap(),
            4950
        );
    });
}

#[test]
fn large_args_ref() {
    Lua::new().context(|lua| {
        let f = lua
            .create_function(|_, args: Variadic<String>| {
                for i in 0..args.len() {
                    assert_eq!(args[i], i.to_string());
                }
                Ok(())
            })
            .unwrap();

        f.call::<_, ()>((0..100).map(|i| i.to_string()).collect::<Variadic<_>>())
            .unwrap();
    });
}

#[test]
fn chunk_env() {
    Lua::new().context(|lua| {
        let assert: Function = lua.globals().get("assert").unwrap();

        let env1 = lua.create_table().unwrap();
        env1.set("assert", assert.clone()).unwrap();

        let env2 = lua.create_table().unwrap();
        env2.set("assert", assert).unwrap();

        lua.load(
            r#"
                test_var = 1
            "#,
        )
        .set_environment(env1.clone())
        .exec()
        .unwrap();

        lua.load(
            r#"
                assert(test_var == nil)
                test_var = 2
            "#,
        )
        .set_environment(env2.clone())
        .exec()
        .unwrap();

        assert_eq!(
            lua.load("test_var")
                .set_environment(env1)
                .eval::<i32>()
                .unwrap(),
            1
        );

        assert_eq!(
            lua.load("test_var")
                .set_environment(env2)
                .eval::<i32>()
                .unwrap(),
            2
        );
    });
}

#[cfg(not(rlua_lua51))]
#[test]
fn context_thread() {
    Lua::new().context(|lua_ctx| {
        let f = lua_ctx
            .load(
                r#"
                    local thread = ...
                    assert(coroutine.running() == thread)
                "#,
            )
            .into_function()
            .unwrap();
        f.call::<_, ()>(lua_ctx.current_thread()).unwrap();
    });
}

#[cfg(rlua_lua51)]
#[test]
fn context_thread_lua51() {
    Lua::new().context(|lua_ctx| {
        let f = lua_ctx
            .load(
                r#"
                    local thread = ...
                    assert(coroutine.running() == thread)
                "#,
            )
            .into_function()
            .unwrap();
        // We pass in nothing as coroutine.running() returns nil in Lua 5.1
        // for the main thread.
        f.call::<_, ()>(()).unwrap();

        let thrd = lua_ctx.create_thread(f).unwrap();
        let thrd_cloned = thrd.clone();
        thrd.resume::<_, ()>(thrd_cloned).unwrap();
    });
}

// Test the lua-no-oslib feature
#[cfg(feature = "lua-no-oslib")]
#[test]
fn test_no_oslib() {
    Lua::new().context(|lua_ctx| {
        let f = lua_ctx
            .load(
                r#"
                    assert(os == nil)
                "#,
            )
            .into_function()
            .unwrap();
        f.call::<_, ()>(()).unwrap();
    });
}

// Test the lua-no-oslib feature
#[cfg(not(feature = "lua-no-oslib"))]
#[test]
fn test_with_oslib() {
    Lua::new().context(|lua_ctx| {
        let f = lua_ctx
            .load(
                r#"
                    local x = os.time()
                    assert(x > 0)
                "#,
            )
            .into_function()
            .unwrap();
        f.call::<_, ()>(()).unwrap();
    });
}
