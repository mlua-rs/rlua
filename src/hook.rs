use std::{slice, ptr, mem, str};
use std::borrow::Cow;
use libc::{self, c_int};
use ffi::{self, lua_State, lua_Debug};
use lua::extra_data;
use util::callback_error;

/// Contains information about the running code at the moments specified when setting the hook.
/// Each field is described in the [Lua 5.3 documentaton][lua_doc].
///
/// Depending on when the hook is called, some fields might not be available. In these cases,
/// integers and booleans might not be valid and/or strings are set to `None`.
///
/// [lua_doc]: https://www.lua.org/manual/5.3/manual.html#lua_Debug
#[derive(Clone, Debug)]
pub struct Debug<'a> {
    pub name: Option<Cow<'a, str>>,
    pub namewhat: Option<Cow<'a, str>>,
    pub what: Option<Cow<'a, str>>,
    pub source: Option<Cow<'a, str>>,
    pub curr_line: u32,
    pub line_defined: u32,
    pub last_line_defined: u32,
    pub num_ups: u32,
    pub num_params: u32,
    pub is_vararg: bool,
    pub is_tailcall: bool,
    pub short_src: Option<Cow<'a, str>>,

    #[doc(hidden)]
    _unused: ()
}

impl<'a> Debug<'a> {
    /// Construct a new `Debug` structure that is not associated with a Lua debug structure. It
    /// involves some string copying.
    pub fn to_owned(&'a self) -> Debug<'static> {
        Debug {
            name: self.name.as_ref().and_then(|s| Some(Cow::Owned(s.as_ref().to_string()))),
            namewhat: self.namewhat.as_ref().and_then(|s| Some(Cow::Owned(s.as_ref().to_string()))),
            what: self.what.as_ref().and_then(|s| Some(Cow::Owned(s.as_ref().to_string()))),
            source: self.source.as_ref().and_then(|s| Some(Cow::Owned(s.as_ref().to_string()))),
            curr_line: self.curr_line,
            line_defined: self.line_defined,
            last_line_defined: self.last_line_defined,
            num_ups: self.num_ups,
            num_params: self.num_params,
            is_vararg: self.is_vararg,
            is_tailcall: self.is_tailcall,
            short_src: self.short_src.as_ref().and_then(|s| Some(Cow::Owned(s.as_ref().to_string()))),
            _unused: ()
        }
    }
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
        if ffi::lua_getinfo(state, cstr!("nSltu"), ar) == 0 {
            rlua_panic!("lua_getinfo failed")
        }

        let extra = &mut *extra_data(state);

        let debug = Debug {
            name: ptr_to_str((*ar).name as *const i8),
            namewhat: ptr_to_str((*ar).namewhat as *const i8),
            what: ptr_to_str((*ar).what as *const i8),
            source: ptr_to_str((*ar).source as *const i8),
            curr_line: (*ar).currentline as u32,
            line_defined: (*ar).linedefined as u32,
            last_line_defined: (*ar).lastlinedefined as u32,
            num_ups: (*ar).nups as u32,
            num_params: (*ar).nparams as u32,
            is_vararg: (*ar).isvararg == 1,
            is_tailcall: (*ar).istailcall == 1,
            short_src: str::from_utf8(mem::transmute((*ar).short_src.as_ref()))
                .and_then(|r| Ok(Some(Cow::from(r))))
                .unwrap_or(None),
            _unused: ()
        };

        let cb = extra.hook_callback
            .as_mut()
            .expect("rlua internal error: no hooks previously set; this is a bug");
        cb(&debug)
    });
}

unsafe fn ptr_to_str<'a>(input: *const i8) -> Option<Cow<'a, str>> {
    if input == ptr::null() || ptr::read(input) == 0 {
        return None;
    }
    let len = libc::strlen(input) as usize;
    let input = slice::from_raw_parts(input as *const u8, len);
    str::from_utf8(input)
        .and_then(|r| Ok(Some(Cow::from(r.trim_right()))))
        .unwrap_or(None)
}
