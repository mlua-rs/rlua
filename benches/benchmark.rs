#[macro_use]
extern crate criterion;
extern crate rlua;

use criterion::Criterion;
use rlua::prelude::*;

fn create_table(lua: Lua) -> Lua {
    {
        lua.create_table().unwrap();
    }
    lua
}

fn create_array(lua: Lua) -> Lua {
    {
        let table = lua.create_table().unwrap();
        for i in 1..11 {
            table.set(i, i).unwrap();
        }
    }
    lua
}

fn create_string_table(lua: Lua) -> Lua {
    {
        let table = lua.create_table().unwrap();
        for &s in &["1", "2", "3", "4", "5", "6", "7", "8", "9", "10"] {
            let s = lua.create_string(s).unwrap();
            table.set(s.clone(), s).unwrap();
        }
    }
    lua
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("create_table", |b| {
        b.iter_with_setup(|| Lua::new(), create_table)
    });
    c.bench_function("create_array", |b| {
        b.iter_with_setup(|| Lua::new(), create_array)
    });
    c.bench_function("create_string_table", |b| {
        b.iter_with_setup(|| Lua::new(), create_string_table)
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
