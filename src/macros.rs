macro_rules! cstr {
  ($s:expr) => (
    concat!($s, "\0") as *const str as *const [::std::os::raw::c_char] as *const ::std::os::raw::c_char
  );
}

macro_rules! abort {
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

macro_rules! rlua_panic {
    ($msg:expr) => {
        panic!(concat!("rlua internal error: ", $msg));
    };

    ($msg:expr, $($arg:tt)+) => {
        panic!(concat!("rlua internal error: ", $msg), $($arg)+);
    };
}

macro_rules! rlua_assert {
    ($cond:expr, $msg:expr) => {
        assert!($cond, concat!("rlua internal error: ", $msg));
    };

    ($cond:expr, $msg:expr, $($arg:tt)+) => {
        assert!($cond, concat!("rlua internal error: ", $msg), $($arg)+);
    };
}

macro_rules! rlua_debug_assert {
    ($cond:expr, $msg:expr) => {
        debug_assert!($cond, concat!("rlua internal error: ", $msg));
    };

    ($cond:expr, $msg:expr, $($arg:tt)+) => {
        debug_assert!($cond, concat!("rlua internal error: ", $msg), $($arg)+);
    };
}

macro_rules! rlua_abort {
    ($msg:expr) => {
        {
            abort!(concat!("rlua internal error: ", $msg));
        }
    };

    ($msg:expr, $($arg:tt)+) => {
        {
            abort!(concat!("rlua internal error, aborting!: ", $msg), $($arg)+);
        }
    };
}
