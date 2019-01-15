use std::borrow::Cow;

use rlua::{Lua, String};

fn with_str<F>(s: &str, f: F)
where
    F: FnOnce(String),
{
    Lua::new().context(|lua| {
        let string = lua.create_string(s).unwrap();
        f(string);
    });
}

#[test]
fn compare() {
    // Tests that all comparisons we want to have are usable
    with_str("teststring", |t| assert_eq!(t, "teststring")); // &str
    with_str("teststring", |t| assert_eq!(t, b"teststring")); // &[u8]
    with_str("teststring", |t| assert_eq!(t, b"teststring".to_vec())); // Vec<u8>
    with_str("teststring", |t| assert_eq!(t, "teststring".to_string())); // String
    with_str("teststring", |t| assert_eq!(t, t)); // rlua::String
    with_str("teststring", |t| {
        assert_eq!(t, Cow::from(b"teststring".as_ref()))
    }); // Cow (borrowed)
    with_str("bla", |t| assert_eq!(t, Cow::from(b"bla".to_vec()))); // Cow (owned)
}

#[test]
fn string_views() {
    Lua::new().context(|lua| {
        lua.load(
            r#"
                ok = "null bytes are valid utf-8, wh\0 knew?"
                err = "but \xff isn't :("
                empty = ""
            "#,
        )
        .exec()
        .unwrap();

        let globals = lua.globals();
        let ok: String = globals.get("ok").unwrap();
        let err: String = globals.get("err").unwrap();
        let empty: String = globals.get("empty").unwrap();

        assert_eq!(
            ok.to_str().unwrap(),
            "null bytes are valid utf-8, wh\0 knew?"
        );
        assert_eq!(
            ok.as_bytes(),
            &b"null bytes are valid utf-8, wh\0 knew?"[..]
        );

        assert!(err.to_str().is_err());
        assert_eq!(err.as_bytes(), &b"but \xff isn't :("[..]);

        assert_eq!(empty.to_str().unwrap(), "");
        assert_eq!(empty.as_bytes_with_nul(), &[0]);
        assert_eq!(empty.as_bytes(), &[]);
    });
}

#[test]
fn raw_string() {
    Lua::new().context(|lua| {
        let rs = lua.create_string(&[0, 1, 2, 3, 0, 1, 2, 3]).unwrap();
        assert_eq!(rs.as_bytes(), &[0, 1, 2, 3, 0, 1, 2, 3]);
    });
}
