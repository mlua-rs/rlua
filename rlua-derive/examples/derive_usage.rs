extern crate rlua;

use rlua::*;

fn derive_usage() -> Result<()> {
    let lua = Lua::new();
    #[derive(LuaTable, Debug)]
    struct Vec2D(f64, f64);

    let table = Vec2D(3.0, 4.0).into_table(&lua)?;
    lua.globals().set("v2", table)?;
    lua.exec::<()>(
        r#"
print(v2._0)
print(v2._1)
v2._0 = 100
v2._1 = v2._0 * 2
"#,
        None,
    )?;

    let table = lua.globals().get("v2")?;
    let v2 = Vec2D::from_table(table, &lua)?;
    println!("{:?}", v2);

    #[derive(LuaTable, Debug)]
    struct Vec3D {
        x: f64,
        y: f64,
        z: f64,
    }
    impl Vec3D {
        fn new<T: Into<f64>>(x: T, y: T, z: T) -> Vec3D {
            Vec3D {
                x: x.into(),
                y: y.into(),
                z: z.into(),
            }
        }
    }
    let table = Vec3D::new(4, 5, 6).into_table(&lua)?;
    lua.globals().set("v3", table)?;
    lua.exec::<()>(
        r#"
print(v3.x)
print(v3.y)
print(v3.z)
v3.x = 356
v3.y = v3.x * 2
v3.z = v3.y * 2
"#,
        None,
    )?;
    let table = lua.globals().get("v3")?;
    let v3 = Vec3D::from_table(table, &lua)?;
    println!("{:?}", v3);
    Ok(())
}

fn main() {
    derive_usage().unwrap();
}
