
use semver::{Version, VersionReq};
use std::env;

fn main() {
    let lua_version = match env::var("LUA_VERSION") {
        Ok(val) => val,
        Err(_e) => panic!("lua version not specified")
    };
    let semvar_lua = Version::parse(&lua_version[1..]).expect("failed to match lua version");
    if VersionReq::parse("5.4").unwrap().matches(&semvar_lua) {
        println!("cargo:rustc-cfg=rlua_lua54");
    } else if VersionReq::parse("5.3").unwrap().matches(&semvar_lua) {
        println!("cargo:rustc-cfg=rlua_lua53");
    }
}
