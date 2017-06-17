extern crate gcc;

use std::env;

fn main() {
    let mut config = gcc::Config::new();

    if env::var("CARGO_CFG_TARGET_OS") == Ok("linux".to_string()) {
        // Among other things, this enables `io.popen` support.
        // Despite the name, we could enable this on more platforms.
        config.define("LUA_USE_LINUX", None);
    }

    config.define("LUA_USE_APICHECK", None)
        .include("lua")
        .file("lua/lapi.c")
        .file("lua/lauxlib.c")
        .file("lua/lbaselib.c")
        .file("lua/lbitlib.c")
        .file("lua/lcode.c")
        .file("lua/lcorolib.c")
        .file("lua/lctype.c")
        .file("lua/ldblib.c")
        .file("lua/ldebug.c")
        .file("lua/ldo.c")
        .file("lua/ldump.c")
        .file("lua/lfunc.c")
        .file("lua/lgc.c")
        .file("lua/linit.c")
        .file("lua/liolib.c")
        .file("lua/llex.c")
        .file("lua/lmathlib.c")
        .file("lua/lmem.c")
        .file("lua/loadlib.c")
        .file("lua/lobject.c")
        .file("lua/lopcodes.c")
        .file("lua/loslib.c")
        .file("lua/lparser.c")
        .file("lua/lstate.c")
        .file("lua/lstring.c")
        .file("lua/lstrlib.c")
        .file("lua/ltable.c")
        .file("lua/ltablib.c")
        .file("lua/ltm.c")
        .file("lua/lundump.c")
        .file("lua/lutf8lib.c")
        .file("lua/lvm.c")
        .file("lua/lzio.c")
        .compile("liblua.a");
}
