#[macro_export]
macro_rules! lua_multi {
    [] => { $crate::LNil };
    [$head: expr] => { $crate::LCons($head, $crate::LNil) };
    [$head: expr, $($tail: expr), *] => { $crate::LCons($head, lua_multi![$($tail), *]) };
    [$head: expr, $($tail: expr), *,] => { $crate::LCons($head, lua_multi![$($tail), *]) };
}

#[macro_export]
macro_rules! lua_multi_pat {
    [] => { $crate::LNil{} };
    [$head: pat] => { $crate::LCons($head, $crate::LNil{}) };
    [$head: pat, $($tail: pat), *] => { $crate::LCons($head, lua_multi_pat![$($tail), *]) };
    [$head: pat, $($tail: pat), *,] => { $crate::LCons($head, lua_multi_pat![$($tail), *]) };
}

#[macro_export]
macro_rules! LuaMulti {
    [] => { $crate::LNil };
    [$head: ty] => { $crate::LCons<$head, $crate::LNil> };
    [$head: ty, $($tail: ty), *] => { $crate::LCons<$head, LuaMulti![$($tail), *]> };
    [$head: ty, $($tail: ty), *,] => { $crate::LCons<$head, LuaMulti![$($tail), *]> };
}

#[macro_export]
macro_rules! lua_cstr {
  ($s:expr) => (
    concat!($s, "\0") as *const str as *const [c_char] as *const c_char
  );
}
