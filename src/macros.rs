macro_rules! cstr {
  ($s:expr) => (
    concat!($s, "\0") as *const str as *const [c_char] as *const c_char
  );
}

// A panic that clears the given lua stack before panicking
macro_rules! lua_panic {
    ($state:expr) => {
        {
            $crate::ffi::lua_settor($state, 0);
            panic!("rlua internal error");
        }
    };

    ($state:expr, $msg:expr) => {
        {
            $crate::ffi::lua_settop($state, 0);
            panic!(concat!("rlua: ", $msg));
        }
    };

    ($state:expr, $fmt:expr, $($arg:tt)+) => {
        {
            $crate::ffi::lua_settop($state, 0);
            panic!(concat!("rlua: ", $fmt), $($arg)+);
        }
    };
}

// An assert that clears the given lua stack before panicking
macro_rules! lua_assert {
    ($state:expr, $cond:expr) => {
        if !$cond {
            $crate::ffi::lua_settop($state, 0);
            panic!("rlua internal error");
        }
    };

    ($state:expr, $cond:expr, $msg:expr) => {
        if !$cond {
            $crate::ffi::lua_settop($state, 0);
            panic!(concat!("rlua: ", $msg));
        }
    };

    ($state:expr, $cond:expr, $fmt:expr, $($arg:tt)+) => {
        if !$cond {
            $crate::ffi::lua_settop($state, 0);
            panic!(concat!("rlua: ", $fmt), $($arg)+);
        }
    };
}

