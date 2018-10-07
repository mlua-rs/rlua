use libc::c_int;
use ffi::{self, lua_State, lua_Debug};
use lua::extra_data;
use util::callback_error;

/// Contains information about the running code at the moments specified when setting the hook.
pub struct Debug {
    pub curr_line: u32
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
        let extra = &mut *extra_data(state);

        let debug = Debug {
            curr_line: (*ar).currentline as u32
        };

        let cb = extra.hook_callback
            .as_mut()
            .expect("no hooks previously set; this is a bug");
        cb(&debug)
    });
}
