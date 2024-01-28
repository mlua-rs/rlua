use rlua::{Integer, Lua, Result, String, Table, Value, RluaCompat, ToLuaCompat};

fn valid_float(verify: Result<Value>, expected: f64) {
    let verify_unwrap = verify.unwrap();
    assert_eq!(verify_unwrap.type_name(), "number");
    match verify_unwrap {
        Value::Number(value) => assert_eq!(value, expected),
        _ => panic!("unexpected type"),
    };
}

#[cfg(rlua_lua51)]
fn valid_int(verify: Result<Value>, expected: Integer) {
    let verify_unwrap = verify.unwrap();
    assert_eq!(verify_unwrap.type_name(), "number");
    match verify_unwrap {
        Value::Number(value) => assert_eq!(value as Integer, expected),
        _ => panic!("unexpected type"),
    };
}

#[cfg(not(rlua_lua51))]
fn valid_int(verify: Result<Value>, expected: Integer) {
    let verify_unwrap = verify.unwrap();
    assert_eq!(verify_unwrap.type_name(), "integer");
    match verify_unwrap {
        Value::Integer(value) => assert_eq!(value, expected),
        _ => panic!("unexpected type"),
    };
}

fn valid_table(verify: Result<Value>, handler: fn(tbl: Table)) {
    let verify_unwrap = verify.unwrap();
    assert_eq!(verify_unwrap.type_name(), "table");
    match verify_unwrap {
        Value::Table(value) => handler(value),
        _ => panic!("unexpected type"),
    };
}

fn valid_string(verify: Result<Value>, val: String) {
    let verify_unwrap = verify.unwrap();
    assert_eq!(verify_unwrap.type_name(), "string");
    match verify_unwrap {
        Value::String(value) => assert_eq!(value, val),
        _ => panic!("unexpected type"),
    };
}

fn valid_boolean(verify: Result<Value>, val: bool) {
    let verify_unwrap = verify.unwrap();
    assert_eq!(verify_unwrap.type_name(), "boolean");
    match verify_unwrap {
        Value::Boolean(value) => assert_eq!(value, val),
        _ => panic!("unexpected type"),
    };
}

#[test]
fn test_conversion_int_primitives() {
    let lua = Lua::new();

    let v: i8 = 10;
    let v2: u8 = 10;
    let v3: i16 = 10;
    let v4: u16 = 10;
    let v5: i32 = 10;
    let v6: u32 = 10;
    let v7: i64 = 10;
    let v8: u64 = 10;
    let v9: i128 = 10;
    let v10: u128 = 10;
    let v11: isize = 10;
    let v12: usize = 10;

    lua.context(|ctx| {
        valid_int(v.to_lua(ctx), 10);
        valid_int(v2.to_lua(ctx), 10);
        valid_int(v3.to_lua(ctx), 10);
        valid_int(v4.to_lua(ctx), 10);
        valid_int(v5.to_lua(ctx), 10);
        valid_int(v6.to_lua(ctx), 10);
        valid_int(v7.to_lua(ctx), 10);
        valid_int(v8.to_lua(ctx), 10);
        valid_int(v9.to_lua(ctx), 10);
        valid_int(v10.to_lua(ctx), 10);
        valid_int(v11.to_lua(ctx), 10);
        valid_int(v12.to_lua(ctx), 10);
    });
}

#[test]
fn test_conversion_float_primatives() {
    let lua = Lua::new();

    let v: f32 = 10.0;
    let v2: f64 = 10.0;

    lua.context(|ctx| {
        valid_float(v.to_lua(ctx), 10.0);
        valid_float(v2.to_lua(ctx), 10.0);
    });
}

#[test]
fn test_conversion_int_array_table() {
    let v1: [u32; 3] = [10, 15, 4];
    let v2: [u8; 3] = [10, 15, 4];
    let v3: [i16; 3] = [10, 15, 4];
    let v4: [u16; 3] = [10, 15, 4];
    let v5: [i32; 3] = [10, 15, 4];
    let v6: [u32; 3] = [10, 15, 4];
    let v7: [i64; 3] = [10, 15, 4];
    let v8: [u64; 3] = [10, 15, 4];
    let v9: [i128; 3] = [10, 15, 4];
    let v10: [u128; 3] = [10, 15, 4];
    let v11: [isize; 3] = [10, 15, 4];
    let v12: [usize; 3] = [10, 15, 4];

    let v1f: [f32; 3] = [10.0, 15.0, 4.0];
    let v2f: [f64; 3] = [10.0, 15.0, 4.0];

    let lua = Lua::new();
    lua.context(|ctx| {
        let validate_arr_int = |tbl: Table| {
            valid_int(tbl.get(1), 10);
            valid_int(tbl.get(2), 15);
            valid_int(tbl.get(3), 4);
        };
        valid_table(v1.to_lua(ctx), validate_arr_int);
        valid_table(v2.to_lua(ctx), validate_arr_int);
        valid_table(v3.to_lua(ctx), validate_arr_int);
        valid_table(v4.to_lua(ctx), validate_arr_int);
        valid_table(v5.to_lua(ctx), validate_arr_int);
        valid_table(v6.to_lua(ctx), validate_arr_int);
        valid_table(v7.to_lua(ctx), validate_arr_int);
        valid_table(v8.to_lua(ctx), validate_arr_int);
        valid_table(v9.to_lua(ctx), validate_arr_int);
        valid_table(v10.to_lua(ctx), validate_arr_int);
        valid_table(v11.to_lua(ctx), validate_arr_int);
        valid_table(v12.to_lua(ctx), validate_arr_int);

        let validate_arr_float = |tbl: Table| {
            valid_float(tbl.get(1), 10.0);
            valid_float(tbl.get(2), 15.0);
            valid_float(tbl.get(3), 4.0);
        };
        valid_table(v1f.to_lua(ctx), validate_arr_float);
        valid_table(v2f.to_lua(ctx), validate_arr_float);
    });
}

#[test]
fn test_conversion_string() {
    Lua::new().context(|ctx| {
        valid_string(
            "hello world".to_lua(ctx),
            ctx.create_string("hello world").unwrap(),
        );
    });
}

#[test]
fn test_conversion_boolean() {
    Lua::new().context(|ctx| {
        valid_boolean(true.to_lua(ctx), true);
    });
}
