#![cfg(feature = "json")]

use rlua::{FromLua as _, Lua, ToLua as _, Value};
use serde_json::{json, Value as JsonValue};

#[test]
fn test_to_nil() {
    Lua::new().context(|lua| match json!(null).to_lua(lua) {
        Ok(Value::Nil) => (),
        Ok(x) => panic!("unexpected conversion result: {:?}", x),
        Err(e) => panic!("conversion error: {}", e),
    });
}

#[test]
fn test_from_nil() {
    Lua::new().context(|lua| match JsonValue::from_lua(Value::Nil, lua) {
        Ok(JsonValue::Null) => (),
        Ok(x) => panic!("unexpected conversion result: {:?}", x),
        Err(e) => panic!("conversion error: {}", e),
    })
}
