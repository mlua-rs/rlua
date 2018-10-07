extern crate rlua;

use std::sync::mpsc::{channel, TryRecvError};
use rlua::{Lua, Debug, HookOptions};

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
