fn main() {
    let mut lua_version_features = 0;
    #[cfg(feature = "rlua_lua54")]
    {
        println!("cargo:rustc-cfg=rlua_lua54");
        lua_version_features += 1;
    }

    #[cfg(feature = "rlua_lua53")]
    {
        println!("cargo:rustc-cfg=rlua_lua53");
        lua_version_features += 1;
    }

    #[cfg(feature = "rlua_lua51")]
    {
        println!("cargo:rustc-cfg=rlua_lua51");
        lua_version_features += 1;
    }
    if lua_version_features < 1 {
        panic!("No Lua version specified.  Please enable one of the features.");
    } else if lua_version_features > 1 {
        panic!("Cannot enable more than one Lua interpreter feature.");
    }
}
