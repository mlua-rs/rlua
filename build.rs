fn main() {
    #[cfg(feature = "rlua_lua54")]
    {
        println!("cargo:rustc-cfg=rlua_lua54");
    }

    #[cfg(feature = "rlua_lua53")]
    {
        println!("cargo:rustc-cfg=rlua_lua53");
    }

    #[cfg(feature = "rlua_lua51")]
    {
        println!("cargo:rustc-cfg=rlua_lua51");
    }
}
