use std::{slice, ptr, mem, str};
use libc::{self, c_int};
use ffi::{self, lua_State, lua_Debug};
use lua::extra_data;
use util::callback_error;

/// Contains information about the running code at the moments specified when setting the hook.
#[derive(Clone, Debug)]
pub struct Debug<'a> {
    pub name: Option<&'a str>,
    pub namewhat: Option<&'a str>,
    pub what: Option<&'a str>,
    pub source: Option<&'a str>,
    pub curr_line: u32,
    pub line_defined: u32,
    pub last_line_defined: u32,
    pub num_ups: u32,
    pub num_params: u32,
    pub is_vararg: bool,
    pub is_tailcall: bool,
    pub short_src: Option<&'a str>
}

/// Indicates in which circumstances the hook should be called by Lua.
pub struct HookOptions {
    /// Before a function call.
    pub calls: bool,
    /// When Lua returns from a function.
    pub returns: bool,
    /// Before executing a new line, or returning from a function call.
    pub lines: bool,
    /// After a certain amount of instructions specified by `count`.
    pub after_counts: bool,
    /// Indicates how many instructions to execute before calling the hook. Only effective when
    /// `after_counts` is set to true.
    pub count: u32
}

impl HookOptions {
    // Computes the mask to pass to `lua_sethook`.
    pub(crate) fn mask(&self) -> c_int {
        let mut mask: c_int = 0;
        if self.calls { mask |= ffi::LUA_MASKCALL }
        if self.returns { mask |= ffi::LUA_MASKRET }
        if self.lines { mask |= ffi::LUA_MASKLINE }
        if self.after_counts { mask |= ffi::LUA_MASKCOUNT }
        mask
    }
}

impl Default for HookOptions {
    fn default() -> Self {
        HookOptions {
            calls: false,
            returns: false,
            lines: false,
            after_counts: false,
            count: 0
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
                .and_then(|r| Ok(Some(r)))
                .unwrap_or(None)
        };

        let cb = extra.hook_callback
            .as_mut()
            .expect("no hooks previously set; this is a bug");
        cb(&debug)
    });
}

unsafe fn ptr_to_str<'a>(input: *const i8) -> Option<&'a str> {
    if input == ptr::null() || ptr::read(input) == 0 {
        return None;
    }
    let len = libc::strlen(input) as usize;
    let input = slice::from_raw_parts(input as *const u8, len);
    str::from_utf8(input).and_then(|r| Ok(Some(r.trim_right()))).unwrap_or(None)
}
