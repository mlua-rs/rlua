use rlua::{Lua, RluaCompat};

#[test]
fn test_to_array() {
    Lua::new().context(|lua| {
        let globals = lua.globals();
        lua.load(
            r#"
                a = { 1, 2, 3, 4 }
            "#,
        )
        .exec()
        .unwrap();
        let res = globals.get::<_, Vec<usize>>("a").unwrap();
        assert_eq!(res, vec![1, 2, 3, 4]);

        let res = globals.get::<_, [usize; 4]>("a").unwrap();
        assert_eq!(res, [1, 2, 3, 4]);

        let res = globals.get::<_, [usize; 3]>("a");
        assert!(res.is_err());
        let res = globals.get::<_, [usize; 5]>("a");
        assert!(res.is_err());
    });
}

#[test]
fn test_from_array() {
    Lua::new().context(|lua| {
        let globals = lua.globals();
        globals.set("a", [1usize, 2, 3]).unwrap();
        globals.set("v", vec![1usize, 2, 3]).unwrap();
        lua.load(
            r#"
                correct = 0
                for i=1, 3 do
                    if a[i] == i then
                        correct = correct + 1
                    end
                    if v[i] == i then
                        correct = correct + 1
                    end
                end
            "#,
        )
        .exec()
        .unwrap();
        let correct = globals.get::<_, usize>("correct").unwrap();
        assert_eq!(correct, 6);
    });
}
