extern crate rlua;

use std::sync::mpsc::{channel, TryRecvError};
use std::ops::Deref;
use rlua::{Lua, Debug, HookOptions, Error};

#[test]
fn line_counts() {
    let code = r#"local x = 2 + 3
    local y = x * 63
    local z = string.len(x..", "..y)
    "#;

    let (sx, rx) = channel();
    let lua = Lua::new();
    lua.set_mut_hook(HookOptions {
        lines: true, ..Default::default()
    }, move |debug: &Debug| {
        let _ = sx.send(debug.curr_line);
        Ok(())
    });
    let _: () = lua.exec(code, None).expect("exec error");

    assert_eq!(rx.try_recv(), Ok(1));
    assert_eq!(rx.try_recv(), Ok(2));
    assert_eq!(rx.try_recv(), Ok(3));
    assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));
}

#[test]
fn function_calls() {
    let code = r#"local v = string.len("Hello World")"#;

    let (sx, rx) = channel();
    let lua = Lua::new();
    lua.set_mut_hook(HookOptions {
        calls: true, ..Default::default()
    }, move |debug: &Debug| {
        let _ = sx.send(debug.to_owned());
        Ok(())
    });
    let _: () = lua.exec(code, None).expect("exec error");

    assert_eq!(rx.recv().unwrap().what.as_ref().unwrap().as_ref(), "main");
    assert_eq!(rx.recv().unwrap().name.as_ref().unwrap().as_ref(), "len");
}

#[test]
fn error_within_hook() {
    let lua = Lua::new();
    lua.set_mut_hook(HookOptions {
        lines: true, ..Default::default()
    }, move |_debug: &Debug| {
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
