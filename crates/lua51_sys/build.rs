extern crate cc;
extern crate git2;

use git2::Repository;
use std::env;
use std::path::PathBuf;

fn main() {
    let git_url = "https://github.com/lua/lua.git";
    let lua_version = "v5.1.1";
    let wrapper_h = "wrapper_lua51.h";

    let dst = PathBuf::from(env::var_os("OUT_DIR").unwrap());

    let mut binding_config = bindgen::Builder::default();

    if cfg!(feature = "lua51-pkg-config") {
        let library = pkg_config::Config::new().probe("lua5.1").unwrap();
        for path in &library.include_paths {
            binding_config = binding_config.clang_arg(format!("-I{}", path.to_string_lossy()));
        }
    } else {
        let lua_dir = dst.join("lua");
        let repo = match lua_dir.exists() {
            true => match Repository::open(&lua_dir) {
                Ok(repo) => repo,
                Err(e) => {
                    panic!("failed to open repo: {}", e)
                }
            },
            false => match Repository::clone(&git_url, &lua_dir) {
                Ok(repo) => repo,
                Err(e) => {
                    panic!("unable to clone repo: {}", e)
                }
            },
        };

        let (object, reference) = repo.revparse_ext(&lua_version).expect("Object not found");
        repo.checkout_tree(&object, None)
            .expect("Failed to checkout");
        match reference {
            // gref is an actual reference like branches or tags
            Some(gref) => repo.set_head(gref.name().unwrap()),
            // this is a commit, not a reference
            None => repo.set_head_detached(object.id()),
        }
        .expect("Failed to set HEAD");

        let target_os = env::var("CARGO_CFG_TARGET_OS");
        let target_family = env::var("CARGO_CFG_TARGET_FAMILY");

        let mut cc_config = cc::Build::new();
        cc_config.warnings(false);

        if target_os == Ok("linux".to_string()) {
            cc_config.define("LUA_USE_LINUX", None);
        } else if target_os == Ok("macos".to_string()) {
            cc_config.define("LUA_USE_MACOSX", None);
        } else if target_family == Ok("unix".to_string()) {
            cc_config.define("LUA_USE_POSIX", None);
        } else if target_family == Ok("windows".to_string()) {
            cc_config.define("LUA_USE_WINDOWS", None);
        }

        let mut cc_config_build = cc_config.include(&lua_dir);

        cc_config_build = cc_config_build
            .file(lua_dir.join("lapi.c"))
            .file(lua_dir.join("lbaselib.c"))
            .file(lua_dir.join("lcode.c"))
            .file(lua_dir.join("ldblib.c"))
            .file(lua_dir.join("ldebug.c"))
            .file(lua_dir.join("ldo.c"))
            .file(lua_dir.join("ldump.c"))
            .file(lua_dir.join("lfunc.c"))
            .file(lua_dir.join("lgc.c"))
            .file(lua_dir.join("linit.c"))
            .file(lua_dir.join("liolib.c"))
            .file(lua_dir.join("llex.c"))
            .file(lua_dir.join("lmathlib.c"))
            .file(lua_dir.join("lauxlib.c"))
            .file(lua_dir.join("lmem.c"))
            .file(lua_dir.join("loadlib.c"))
            .file(lua_dir.join("lobject.c"))
            .file(lua_dir.join("lopcodes.c"))
            .file(lua_dir.join("loslib.c"))
            .file(lua_dir.join("lparser.c"))
            .file(lua_dir.join("lstate.c"))
            .file(lua_dir.join("lstring.c"))
            .file(lua_dir.join("lstrlib.c"))
            .file(lua_dir.join("ltable.c"))
            .file(lua_dir.join("ltablib.c"))
            .file(lua_dir.join("ltests.c"))
            .file(lua_dir.join("ltm.c"))
            .file(lua_dir.join("lundump.c"))
            .file(lua_dir.join("lvm.c"))
            .file(lua_dir.join("lzio.c"));

        cc_config_build
            .out_dir(dst.join("lib"))
            .compile("liblua5.1.a");

        binding_config = binding_config.clang_arg(format!("-I{}", lua_dir.to_string_lossy()));
    }

    println!("cargo:rerun-if-changed={}", wrapper_h);
    let bindings = binding_config
        .size_t_is_usize(true)
        .header(wrapper_h)
        .clang_arg("-x")
        .clang_arg("c")
        .generate()
        .expect("Unable to generate bindings");

    bindings
        .write_to_file(dst.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
