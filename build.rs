fn main() {
    let mut lua_version_features = 0;
    if cfg!(feature = "system-lua51") {
        lua_version_features += 1;
    }
    if cfg!(feature = "system-lua53") {
        lua_version_features += 1;
    }
    if cfg!(feature = "system-lua54") {
        lua_version_features += 1;
    }
    if cfg!(feature = "builtin-lua53") {
        lua_version_features += 1;
    }
    if cfg!(feature = "builtin-lua54") {
        lua_version_features += 1;
    }
    if lua_version_features < 1 {
        panic!("No Lua version specified.  Please enable one of the features.");
    } else if lua_version_features > 1 {
        panic!("Cannot enable more than one Lua interpreter feature.");
    }

    #[cfg(feature = "builtin-lua54")]
    {
        use std::env;

        let target_os = env::var("CARGO_CFG_TARGET_OS");
        let target_family = env::var("CARGO_CFG_TARGET_FAMILY");

        let mut config = cc::Build::new();

        if target_os == Ok("linux".to_string()) {
            config.define("LUA_USE_LINUX", None);
        } else if target_os == Ok("macos".to_string()) {
            config.define("LUA_USE_MACOSX", None);
        } else if target_family == Ok("unix".to_string()) {
            config.define("LUA_USE_POSIX", None);
        } else if target_family == Ok("windows".to_string()) {
            config.define("LUA_USE_WINDOWS", None);
        }

        if cfg!(debug_assertions) {
            config.define("LUA_USE_APICHECK", None);
        }

        config
            .include("lua5.4")
            .file("lua5.4/lapi.c")
            .file("lua5.4/lauxlib.c")
            .file("lua5.4/lbaselib.c")
            .file("lua5.4/lcode.c")
            .file("lua5.4/lcorolib.c")
            .file("lua5.4/lctype.c")
            .file("lua5.4/ldblib.c")
            .file("lua5.4/ldebug.c")
            .file("lua5.4/ldo.c")
            .file("lua5.4/ldump.c")
            .file("lua5.4/lfunc.c")
            .file("lua5.4/lgc.c")
            .file("lua5.4/linit.c")
            .file("lua5.4/liolib.c")
            .file("lua5.4/llex.c")
            .file("lua5.4/lmathlib.c")
            .file("lua5.4/lmem.c")
            .file("lua5.4/loadlib.c")
            .file("lua5.4/lobject.c")
            .file("lua5.4/lopcodes.c")
            .file("lua5.4/loslib.c")
            .file("lua5.4/lparser.c")
            .file("lua5.4/lstate.c")
            .file("lua5.4/lstring.c")
            .file("lua5.4/lstrlib.c")
            .file("lua5.4/ltable.c")
            .file("lua5.4/ltablib.c")
            .file("lua5.4/ltm.c")
            .file("lua5.4/lundump.c")
            .file("lua5.4/lutf8lib.c")
            .file("lua5.4/lvm.c")
            .file("lua5.4/lzio.c")
            .compile("liblua5.4.a");
        println!("cargo:rustc-cfg=rlua_lua54");
    }

    #[cfg(feature = "builtin-lua53")]
    {
        use std::env;

        let target_os = env::var("CARGO_CFG_TARGET_OS");
        let target_family = env::var("CARGO_CFG_TARGET_FAMILY");

        let mut config = cc::Build::new();

        if target_os == Ok("linux".to_string()) {
            config.define("LUA_USE_LINUX", None);
        } else if target_os == Ok("macos".to_string()) {
            config.define("LUA_USE_MACOSX", None);
        } else if target_family == Ok("unix".to_string()) {
            config.define("LUA_USE_POSIX", None);
        } else if target_family == Ok("windows".to_string()) {
            config.define("LUA_USE_WINDOWS", None);
        }

        if cfg!(debug_assertions) {
            config.define("LUA_USE_APICHECK", None);
        }

        config
            .include("lua5.3/src")
            .file("lua5.3/src/lapi.c")
            .file("lua5.3/src/lauxlib.c")
            .file("lua5.3/src/lbaselib.c")
            .file("lua5.3/src/lbitlib.c")
            .file("lua5.3/src/lcode.c")
            .file("lua5.3/src/lcorolib.c")
            .file("lua5.3/src/lctype.c")
            .file("lua5.3/src/ldblib.c")
            .file("lua5.3/src/ldebug.c")
            .file("lua5.3/src/ldo.c")
            .file("lua5.3/src/ldump.c")
            .file("lua5.3/src/lfunc.c")
            .file("lua5.3/src/lgc.c")
            .file("lua5.3/src/linit.c")
            .file("lua5.3/src/liolib.c")
            .file("lua5.3/src/llex.c")
            .file("lua5.3/src/lmathlib.c")
            .file("lua5.3/src/lmem.c")
            .file("lua5.3/src/loadlib.c")
            .file("lua5.3/src/lobject.c")
            .file("lua5.3/src/lopcodes.c")
            .file("lua5.3/src/loslib.c")
            .file("lua5.3/src/lparser.c")
            .file("lua5.3/src/lstate.c")
            .file("lua5.3/src/lstring.c")
            .file("lua5.3/src/lstrlib.c")
            .file("lua5.3/src/ltable.c")
            .file("lua5.3/src/ltablib.c")
            .file("lua5.3/src/ltm.c")
            .file("lua5.3/src/lundump.c")
            .file("lua5.3/src/lutf8lib.c")
            .file("lua5.3/src/lvm.c")
            .file("lua5.3/src/lzio.c")
            .compile("liblua5.3.a");
        println!("cargo:rustc-cfg=rlua_lua53");
    }

    #[cfg(feature = "system-lua51")]
    {
        pkg_config::Config::new().probe("lua5.1").unwrap();
        println!("cargo:rustc-cfg=rlua_lua51");
    }

    #[cfg(feature = "system-lua53")]
    {
        pkg_config::Config::new().probe("lua5.3").unwrap();
        println!("cargo:rustc-cfg=rlua_lua53");
    }

    #[cfg(feature = "system-lua54")]
    {
        pkg_config::Config::new().probe("lua5.4").unwrap();
        println!("cargo:rustc-cfg=rlua_lua54");
    }
}
