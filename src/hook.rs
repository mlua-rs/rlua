use std::{slice, ptr, str};
use std::marker::PhantomData;
use libc::{self, c_int};
use ffi::{self, lua_State, lua_Debug};
use lua::{Lua, extra_data};
use util::callback_error;

/// Contains information about the running code at the moments specified when setting the hook.
/// All the documentation can be read in the [Lua 5.3 documentaton][lua_doc].
///
/// # Usage
///
/// For optimal performance, the user must call the methods that return the information they need to
/// use. Those methods return a short structure with the data in the fields. That being said,
/// depending on where the hook is called, some methods might return zeroes and empty strings or
/// possibly bad values.
///
/// This structure does not cache the data obtained; you should only call each function once and
/// keep the reference obtained for as long as needed. However, you may not outlive the hook
/// callback; if you need to, please clone the internals that you need and move them outside.
///
/// # Panics
///
/// This structure contains methods that will panic if there is an internal error. If this happens
/// to you, please make an issue to rlua's repository. as this behavior isn't normal.
///
/// [lua_doc]: https://www.lua.org/manual/5.3/manual.html#lua_Debug
#[derive(Clone)]
pub struct Debug<'a> {
    ar: *mut lua_Debug,
    state: *mut lua_State,
    _phantom: PhantomData<&'a ()>
}

impl<'a> Debug<'a> {
    /// Corresponds to the `n` what mask.
    pub fn names(&self) -> Names<'a> {
        unsafe {
            if ffi::lua_getinfo(self.state, cstr!("n"), self.ar) == 0 {
                rlua_panic!("lua_getinfo failed with `n`")
            } else {
                Names {
                    name: ptr_to_str((*self.ar).name as *const i8),
                    name_what: ptr_to_str((*self.ar).namewhat as *const i8)
                }
            }
        }
    }

    /// Corresponds to the `n` what mask.
    pub fn source(&self) -> Source<'a> {
        unsafe {
            if ffi::lua_getinfo(self.state, cstr!("S"), self.ar) == 0 {
                rlua_panic!("lua_getinfo failed with `S`")
            } else {
                Source {
                    source: ptr_to_str((*self.ar).source as *const i8),
                    short_src: ptr_to_str((*self.ar).short_src.as_ptr() as *const i8),
                    line_defined: (*self.ar).linedefined as i32,
                    last_line_defined: (*self.ar).lastlinedefined as i32,
                    what: ptr_to_str((*self.ar).what as *const i8),
                }
            }
        }
    }

    /// Corresponds to the `l` what mask. Returns the current line.
    pub fn curr_line(&self) -> i32 {
        unsafe {
            if ffi::lua_getinfo(self.state, cstr!("l"), self.ar) == 0 {
                rlua_panic!("lua_getinfo failed with `l`")
            } else {
                (*self.ar).currentline as i32
            }
        }
    }

    /// Corresponds to the `t` what mask. Returns true if the hook is in a function tail call, false
    /// otherwise.
    pub fn is_tail_call(&self) -> bool {
        unsafe {
            if ffi::lua_getinfo(self.state, cstr!("t"), self.ar) == 0 {
                rlua_panic!("lua_getinfo failed with `t`")
            } else {
                (*self.ar).currentline != 0
            }
        }
    }

    /// Corresponds to the `u` what mask.
    pub fn stack(&self) -> Stack {
        unsafe {
            if ffi::lua_getinfo(self.state, cstr!("u"), self.ar) == 0 {
                rlua_panic!("lua_getinfo failed with `u`")
            } else {
                Stack {
                    num_ups: (*self.ar).nups as i32,
                    num_params: (*self.ar).nparams as i32,
                    is_vararg: (*self.ar).isvararg != 0
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Names<'a> {
    pub name: Option<&'a str>,
    pub name_what: Option<&'a str>
}

#[derive(Clone, Debug)]
pub struct Source<'a> {
    pub source: Option<&'a str>,
    pub short_src: Option<&'a str>,
    pub line_defined: i32,
    pub last_line_defined: i32,
    pub what: Option<&'a str>
}

#[derive(Copy, Clone, Debug)]
pub struct Stack {
    pub num_ups: i32,
    pub num_params: i32,
    pub is_vararg: bool
}

/// Indicate in which circumstances the hook should be called by Lua.
///
/// # Usage
///
/// In order to clearly show which fields you are setting to `true` or `Some`, it is highly
/// recommended you use the `Default` trait to fill in any remaining fields. This will also cover
/// you in case new hook features gets added to Lua.
///
/// # Example
///
/// Constructs a `HookTriggers` structure that tells Lua to call the hook after every instruction.
/// Note the use of `Default`.
///
/// ```
/// # use rlua::HookTriggers;
/// # fn main() {
/// let triggers = HookTriggers {
///     every_nth_instruction: Some(1), ..Default::default()
/// };
/// # let _ = triggers;
/// # }
/// ```
pub struct HookTriggers {
    /// Before a function call.
    pub on_calls: bool,
    /// When Lua returns from a function.
    pub on_returns: bool,
    /// Before executing a new line, or returning from a function call.
    pub every_line: bool,
    /// After a certain amount of instructions. When set to `Some(count)`, `count` is the number of
    /// instructions to execute before calling the hook.
    ///
    /// # Performance
    ///
    /// Setting this to a low value will certainly result in a large overhead due to the crate's
    /// safety layer and convenience wrapped over Lua's low-level hooks. `1` is such an example of
    /// a high overhead choice.
    ///
    /// Please find a number that is high enough so it's not that bad of an issue, while still
    /// having enough precision for your needs. Keep in mind instructions are additions, calls to
    /// functions, assignments to variables, etc.; they are very short.
    pub every_nth_instruction: Option<u32>,
}

impl HookTriggers {
    /// Compute the mask to pass to `lua_sethook`.
    pub(crate) fn mask(&self) -> c_int {
        let mut mask: c_int = 0;
        if self.on_calls { mask |= ffi::LUA_MASKCALL }
        if self.on_returns { mask |= ffi::LUA_MASKRET }
        if self.every_line { mask |= ffi::LUA_MASKLINE }
        if self.every_nth_instruction.is_some() { mask |= ffi::LUA_MASKCOUNT }
        mask
    }

    /// Returns the `count` parameter to pass to `lua_sethook`, if applicable. Otherwise, zero is
    /// returned.
    pub(crate) fn count(&self) -> c_int {
        self.every_nth_instruction.unwrap_or(0) as c_int
    }
}

impl Default for HookTriggers {
    fn default() -> Self {
        HookTriggers {
            on_calls: false,
            on_returns: false,
            every_line: false,
            every_nth_instruction: None
        }
    }
}

/// This callback is passed to `lua_sethook` and gets called whenever debug information is received.
pub(crate) unsafe extern "C" fn hook_proc(state: *mut lua_State, ar: *mut lua_Debug) {
    callback_error(state, || {
        let lua = Lua::make_ephemeral(state);
        let debug = Debug {
            ar, state, _phantom: PhantomData
        };

        let cb = (&*extra_data(state)).hook_callback
            .as_ref()
            .map(|rc| rc.clone())
            .expect("rlua internal error: no hooks previously set; this is a bug");
        let outcome = match cb.try_borrow_mut() {
            Ok(mut b) => (&mut *b)(&lua, &debug),
            Err(_) => rlua_panic!("Lua should not allow hooks to be called within another hook;\
                please make an issue")
        };
        outcome
    });
}

unsafe fn ptr_to_str<'a>(input: *const i8) -> Option<&'a str> {
    if input == ptr::null() || ptr::read(input) == 0 {
        return None;
    }
    let len = libc::strlen(input) as usize;
    let input = slice::from_raw_parts(input as *const u8, len);
    str::from_utf8(input)
        .map(|s| Some(s.trim_right()))
        .unwrap_or(None)
}
