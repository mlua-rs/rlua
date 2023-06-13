fn main() {
    let mut lua_version_features = 0;
    #[cfg(feature = "builtin-lua54")]
    {
        println!("cargo:rustc-cfg=rlua_lua54");
        lua_version_features += 1;
    }

    #[cfg(feature = "builtin-lua53")]
    {
        println!("cargo:rustc-cfg=rlua_lua53");
        lua_version_features += 1;
    }

    #[cfg(feature = "builtin-lua51")]
    {
        println!("cargo:rustc-cfg=rlua_lua51");
        lua_version_features += 1;
    }

    #[cfg(feature = "system-lua54")]
    {
        println!("cargo:rustc-cfg=rlua_lua54");
        lua_version_features += 1;
    }

    #[cfg(feature = "system-lua53")]
    {
        println!("cargo:rustc-cfg=rlua_lua53");
        lua_version_features += 1;
    }

    #[cfg(feature = "system-lua51")]
    {
        println!("cargo:rustc-cfg=rlua_lua51");
        lua_version_features += 1;
    }

    #[cfg(feature = "system-luajit")]
    {
        println!("cargo:rustc-cfg=rlua_lua51");
        println!("cargo:rustc-cfg=rlua_luajit");
        lua_version_features += 1;
    }

    if lua_version_features < 1 {
        panic!("No Lua version specified.  Please enable one of the features. use --no-default-features to disable default lua feature.");
    } else if lua_version_features > 1 {
        panic!("Cannot enable more than one Lua interpreter feature.");
    }
}
