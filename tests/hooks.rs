extern crate rlua;

use std::sync::mpsc::{channel, TryRecvError};
use std::ops::Deref;
use std::time::{Instant, Duration};
use rlua::{Lua, Error, Value};
use rlua::hook::{Debug, HookTriggers};

#[test]
fn line_counts() {
    let code = r#"
        local x = 2 + 3
        local y = x * 63
        local z = string.len(x..", "..y)
    "#;

    let (sx, rx) = channel();
    let lua = Lua::new();
    lua.set_hook(HookTriggers {
        every_line: true, ..Default::default()
    }, move |_lua, debug: &Debug| {
        sx.send(debug.curr_line()).unwrap();
        Ok(())
    });
    let _: () = lua.exec(code, None).expect("exec error");

    assert_eq!(rx.try_recv(), Ok(2));
    assert_eq!(rx.try_recv(), Ok(3));
    assert_eq!(rx.try_recv(), Ok(4));
    assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));
}

#[test]
fn function_calls() {
    let code = r#"local v = string.len("Hello World")"#;

    let (sx, rx) = channel();
    let lua = Lua::new();
    lua.set_hook(HookTriggers {
        on_calls: true, ..Default::default()
    }, move |_lua, debug: &Debug| {
        let names = debug.names();
        let source = debug.source();
        let name = names.name.map(|s| s.to_string());
        let what = source.what.map(|s| s.to_string());
        sx.send((name, what)).unwrap();
        Ok(())
    });
    let _: () = lua.exec(code, None).expect("exec error");

    assert_eq!(rx.recv().unwrap(), (None, Some("main".to_string())));
    assert_eq!(rx.recv().unwrap(), (Some("len".to_string()), Some("C".to_string())));
}

#[test]
fn error_within_hook() {
    let lua = Lua::new();
    lua.set_hook(HookTriggers {
        every_line: true, ..Default::default()
    }, |_lua, _debug: &Debug| {
        Err(Error::RuntimeError("Something happened in there!".to_string()))
    });

    let err = lua.exec::<_, ()>("x = 1", None).expect_err("panic didn't propagate");
    match err {
        Error::CallbackError { cause, .. } => match cause.deref() {
            Error::RuntimeError(s) => assert_eq!(s, "Something happened in there!"),
            _ => panic!("wrong callback error kind caught")
        },
        _ => panic!("wrong error kind caught")
    }
}

#[test]
fn limit_execution_time() {
    let code = r#"
        while true do
            x = x + 1
        end
    "#;
    let start = Instant::now();

    let lua = Lua::new();
    lua.globals().set("x", Value::Integer(0)).unwrap();
    lua.set_hook(HookTriggers {
        every_nth_instruction: Some(30), ..Default::default()
    }, move |_lua, _debug: &Debug| {
        if start.elapsed() >= Duration::from_millis(500) {
            Err(Error::RuntimeError("time's up".to_string()))
        } else {
            Ok(())
        }
    });

    let _ = lua.exec::<_, ()>(code, None).expect_err("timeout didn't occur");
    assert!(start.elapsed() < Duration::from_millis(750));
    //println!("{}", lua.globals().get::<_, i64>("x").unwrap());
}

#[test]
fn hook_removal() {
    let code = r#"local x = 1"#;
    let lua = Lua::new();

    lua.set_hook(HookTriggers {
        every_nth_instruction: Some(1), ..Default::default()
    }, |_lua, _debug: &Debug| {
        Err(Error::RuntimeError("this hook should've been removed by this time".to_string()))
    });
    assert!(lua.exec::<_, ()>(code, None).is_err());

    lua.remove_hook();
    assert!(lua.exec::<_, ()>(code, None).is_ok());
}

#[test]
fn hook_swap_within_hook() {
    let code = r#"
        local x = 1
        x = 2
        local y = 3
    "#.trim_left_matches("\r\n");
    let inc_code = r#"if ok ~= nil then ok = ok + 1 end"#;
    let lua = Lua::new();

    lua.set_hook(HookTriggers {
        every_line: true, ..Default::default()
    }, move |lua: &Lua, _debug| {
        lua.globals().set("ok", 1i64).unwrap();
        lua.set_hook(HookTriggers {
            every_line: true, ..Default::default()
        }, move |lua: &Lua, _debug| {
            let _: () = lua.exec(inc_code, Some("hook_incrementer"))
                .expect("exec failure within hook");
            lua.remove_hook();
            Ok(())
        });
        Ok(())
    });

    assert!(lua.exec::<_, ()>(code, None).is_ok());
    assert_eq!(lua.globals().get::<_, i64>("ok").unwrap_or(-1), 2);
}

