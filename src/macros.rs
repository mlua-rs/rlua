macro_rules! cstr {
  ($s:expr) => (
    concat!($s, "\0") as *const str as *const [::std::os::raw::c_char] as *const ::std::os::raw::c_char
  );
}

// A panic that clears the given lua stack before panicking
macro_rules! lua_panic {
    ($state:expr, $msg:expr) => {
        {
            $crate::ffi::lua_settop($state, 0);
            panic!($msg);
        }
    };

    ($state:expr, $msg:expr, $($arg:tt)+) => {
        {
            $crate::ffi::lua_settop($state, 0);
            panic!($msg, $($arg)+);
        }
    };
}

// An assert that clears the given lua stack before panicking
macro_rules! lua_assert {
    ($state:expr, $cond:expr, $msg:expr) => {
        if !$cond {
            $crate::ffi::lua_settop($state, 0);
            panic!($msg);
        }
    };

    ($state:expr, $cond:expr, $msg:expr, $($arg:tt)+) => {
        if !$cond {
            $crate::ffi::lua_settop($state, 0);
            panic!($msg, $($arg)+);
        }
    };
}

macro_rules! lua_abort {
    ($msg:expr) => {
        {
            eprintln!($msg);
            ::std::process::abort()
        }
    };

    ($msg:expr, $($arg:tt)+) => {
        {
            eprintln!($msg, $($arg)+);
            ::std::process::abort()
        }
    };
}

macro_rules! lua_internal_panic {
    ($state:expr, $msg:expr) => {
        lua_panic!($state, concat!("rlua internal error: ", $msg));
    };

    ($state:expr, $msg:expr, $($arg:tt)+) => {
        lua_panic!($state, concat!("rlua internal error: ", $msg), $($arg)+);
    };
}

macro_rules! lua_internal_assert {
    ($state:expr, $cond:expr, $msg:expr) => {
        lua_assert!($state, $cond, concat!("rlua internal error: ", $msg));
    };

    ($state:expr, $cond:expr, $msg:expr, $($arg:tt)+) => {
        lua_assert!($state, $cond, concat!("rlua internal error: ", $msg), $($arg)+);
    };
}

macro_rules! lua_internal_abort {
    ($msg:expr) => {
        {
            lua_abort!(concat!("rlua internal error: ", $msg));
        }
    };

    ($msg:expr, $($arg:tt)+) => {
        {
            lua_abort!(concat!("rlua internal error, aborting!: ", $msg), $($arg)+);
        }
    };
}
