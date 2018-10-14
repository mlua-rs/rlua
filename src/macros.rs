macro_rules! cstr {
    ($s:expr) => {
        concat!($s, "\0") as *const str as *const [::std::os::raw::c_char]
            as *const ::std::os::raw::c_char
    };
}

macro_rules! abort {
    ($msg:expr) => {
        {
            eprintln!($msg);
            ::std::process::abort()
        }
    };

    ($msg:expr,) => {
        abort!($msg);
    };

    ($msg:expr, $($arg:tt)+) => {
        {
            eprintln!($msg, $($arg)+);
            ::std::process::abort()
        }
    };

    ($msg:expr, $($arg:tt)+,) => {
        abort!($msg, $($arg)+);
    };
}

macro_rules! rlua_panic {
    ($msg:expr) => {
        panic!(concat!("rlua internal error: ", $msg));
    };

    ($msg:expr,) => {
        rlua_panic!($msg);
    };

    ($msg:expr, $($arg:tt)+) => {
        panic!(concat!("rlua internal error: ", $msg), $($arg)+);
    };

    ($msg:expr, $($arg:tt)+,) => {
        rlua_panic!($msg, $($arg)+);
    };
}

macro_rules! rlua_assert {
    ($cond:expr, $msg:expr) => {
        assert!($cond, concat!("rlua internal error: ", $msg));
    };

    ($cond:expr, $msg:expr,) => {
        rlua_assert!($cond, $msg);
    };

    ($cond:expr, $msg:expr, $($arg:tt)+) => {
        assert!($cond, concat!("rlua internal error: ", $msg), $($arg)+);
    };

    ($cond:expr, $msg:expr, $($arg:tt)+,) => {
        rlua_assert!($cond, $msg, $($arg)+);
    };
}

macro_rules! rlua_debug_assert {
    ($cond:expr, $msg:expr) => {
        debug_assert!($cond, concat!("rlua internal error: ", $msg));
    };

    ($cond:expr, $msg:expr,) => {
        rlua_debug_assert!($cond, $msg);
    };

    ($cond:expr, $msg:expr, $($arg:tt)+) => {
        debug_assert!($cond, concat!("rlua internal error: ", $msg), $($arg)+);
    };

    ($cond:expr, $msg:expr, $($arg:tt)+,) => {
        rlua_debug_assert!($cond, $msg, $($arg)+);
    };
}

macro_rules! rlua_abort {
    ($msg:expr) => {
        {
            abort!(concat!("rlua internal error: ", $msg));
        }
    };

    ($msg:expr,) => {
        rlua_abort!($msg);
    };

    ($msg:expr, $($arg:tt)+) => {
        {
            abort!(concat!("rlua internal error, aborting!: ", $msg), $($arg)+);
        }
    };

    ($msg:expr, $($arg:tt)+,) => {
        rlua_abort!($msg, $($arg)+);
    };
}

macro_rules! rlua_expect {
    ($res:expr, $msg:expr) => {
        $res.expect(concat!("rlua internal error: ", $msg))
    };

    ($res:expr, $msg:expr,) => {
        rlua_expect!($res, $msg)
    };
}
