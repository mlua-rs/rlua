use criterion::{criterion_group, criterion_main, Criterion};

use rlua::prelude::*;

fn create_table(c: &mut Criterion) {
    c.bench_function("create table", |b| {
        b.iter_with_setup(
            || Lua::new(),
            |lua| -> Lua {
                lua.context(|ctx| {
                    ctx.create_table().unwrap();
                });
                lua
            },
        );
    });
}

fn create_array(c: &mut Criterion) {
    c.bench_function("create array 10", |b| {
        b.iter_with_setup(
            || Lua::new(),
            |lua| -> Lua {
                lua.context(|ctx| {
                    let table = ctx.create_table().unwrap();
                    for i in 1..11 {
                        table.set(i, i).unwrap();
                    }
                });
                lua
            },
        );
    });
}

fn create_string_table(c: &mut Criterion) {
    c.bench_function("create string table 10", |b| {
        b.iter_with_setup(
            || Lua::new(),
            |lua| -> Lua {
                lua.context(|ctx| {
                    let table = ctx.create_table().unwrap();
                    for &s in &["1", "2", "3", "4", "5", "6", "7", "8", "9", "10"] {
                        let s = ctx.create_string(s).unwrap();
                        table.set(s.clone(), s).unwrap();
                    }
                });
                lua
            },
        );
    });
}

fn call_add_function(c: &mut Criterion) {
    c.bench_function("call add function 3 10", |b| {
        b.iter_with_setup(
            || {
                let lua = Lua::new();
                let f = lua.context(|ctx| {
                    let f: LuaFunction = ctx
                        .load(
                            r#"
                                function(a, b, c)
                                    return a + b + c
                                end
                            "#,
                        )
                        .eval()
                        .unwrap();
                    ctx.create_registry_value(f).unwrap()
                });
                (lua, f)
            },
            |(lua, f)| -> Lua {
                lua.context(|ctx| {
                    let add_function: LuaFunction = ctx.registry_value(&f).unwrap();
                    for i in 0..10 {
                        let _result: i64 = add_function.call((i, i + 1, i + 2)).unwrap();
                    }
                });
                lua
            },
        );
    });
}

fn call_add_callback(c: &mut Criterion) {
    c.bench_function("call callback add 2 10", |b| {
        b.iter_with_setup(
            || {
                let lua = Lua::new();
                let f = lua.context(|ctx| {
                    let c: LuaFunction = ctx
                        .create_function(|_, (a, b, c): (i64, i64, i64)| Ok(a + b + c))
                        .unwrap();
                    ctx.globals().set("callback", c).unwrap();
                    let f: LuaFunction = ctx
                        .load(
                            r#"
                                function()
                                    for i = 1,10 do
                                        callback(i, i, i)
                                    end
                                end
                            "#,
                        )
                        .eval()
                        .unwrap();
                    ctx.create_registry_value(f).unwrap()
                });
                (lua, f)
            },
            |(lua, f)| -> Lua {
                lua.context(|ctx| {
                    let entry_function: LuaFunction = ctx.registry_value(&f).unwrap();
                    entry_function.call::<_, ()>(()).unwrap();
                });
                lua
            },
        );
    });
}

fn call_append_callback(c: &mut Criterion) {
    c.bench_function("call callback append 10", |b| {
        b.iter_with_setup(
            || {
                let lua = Lua::new();
                let f = lua.context(|ctx| {
                    let c: LuaFunction = ctx
                        .create_function(|_, (a, b): (LuaString, LuaString)| {
                            Ok(format!("{}{}", a.to_str()?, b.to_str()?))
                        })
                        .unwrap();
                    ctx.globals().set("callback", c).unwrap();
                    let f: LuaFunction = ctx
                        .load(
                            r#"
                                function()
                                    for _ = 1,10 do
                                        callback("a", "b")
                                    end
                                end
                            "#,
                        )
                        .eval()
                        .unwrap();
                    ctx.create_registry_value(f).unwrap()
                });
                (lua, f)
            },
            |(lua, f)| -> Lua {
                lua.context(|ctx| {
                    let entry_function: LuaFunction = ctx.registry_value(&f).unwrap();
                    entry_function.call::<_, ()>(()).unwrap();
                });
                lua
            },
        );
    });
}

fn create_registry_values(c: &mut Criterion) {
    c.bench_function("create registry 10", |b| {
        b.iter_with_setup(
            || Lua::new(),
            |lua| -> Lua {
                lua.context(|ctx| {
                    for _ in 0..10 {
                        ctx.create_registry_value(ctx.pack(true).unwrap()).unwrap();
                    }
                    ctx.expire_registry_values();
                });
                lua
            },
        );
    });
}

fn create_userdata(c: &mut Criterion) {
    struct UserData(i64);
    impl LuaUserData for UserData {}

    c.bench_function("create userdata 10", |b| {
        b.iter_with_setup(
            || Lua::new(),
            |lua| -> Lua {
                lua.context(|ctx| {
                    let table: LuaTable = ctx.create_table().unwrap();
                    for i in 1..11 {
                        table.set(i, UserData(i)).unwrap();
                    }
                });
                lua
            },
        );
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(200)
        .noise_threshold(0.02);
    targets =
        create_table,
        create_array,
        create_string_table,
        call_add_function,
        call_add_callback,
        call_append_callback,
        create_registry_values,
        create_userdata
}

criterion_main!(benches);
