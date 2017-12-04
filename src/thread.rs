use std::os::raw::c_int;

use ffi;
use error::*;
use util::*;
use types::LuaRef;
use value::{FromLuaMulti, MultiValue, ToLuaMulti};

/// Status of a Lua thread (or coroutine).
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ThreadStatus {
    /// The thread was just created, or is suspended because it has called `coroutine.yield`.
    ///
    /// If a thread is in this state, it can be resumed by calling [`Thread::resume`].
    ///
    /// [`Thread::resume`]: struct.Thread.html#method.resume
    Resumable,
    /// Either the thread has finished executing, or the thread is currently running.
    Unresumable,
    /// The thread has raised a Lua error during execution.
    Error,
}

/// Handle to an internal Lua thread (or coroutine).
#[derive(Clone, Debug)]
pub struct Thread<'lua>(pub(crate) LuaRef<'lua>);

impl<'lua> Thread<'lua> {
    /// Resumes execution of this thread.
    ///
    /// Equivalent to `coroutine.resume`.
    ///
    /// Passes `args` as arguments to the thread. If the coroutine has called `coroutine.yield`, it
    /// will return these arguments. Otherwise, the coroutine wasn't yet started, so the arguments
    /// are passed to its main function.
    ///
    /// If the thread is no longer in `Active` state (meaning it has finished execution or
    /// encountered an error), this will return `Err(CoroutineInactive)`, otherwise will return `Ok`
    /// as follows:
    ///
    /// If the thread calls `coroutine.yield`, returns the values passed to `yield`. If the thread
    /// `return`s values from its main function, returns those.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate rlua;
    /// # use rlua::{Lua, Thread, Error, Result};
    /// # fn try_main() -> Result<()> {
    /// let lua = Lua::new();
    /// let thread: Thread = lua.eval(r#"
    ///     coroutine.create(function(arg)
    ///         assert(arg == 42)
    ///         local yieldarg = coroutine.yield(123)
    ///         assert(yieldarg == 43)
    ///         return 987
    ///     end)
    /// "#, None).unwrap();
    ///
    /// assert_eq!(thread.resume::<_, u32>(42).unwrap(), 123);
    /// assert_eq!(thread.resume::<_, u32>(43).unwrap(), 987);
    ///
    /// // The coroutine has now returned, so `resume` will fail
    /// match thread.resume::<_, u32>(()) {
    ///     Err(Error::CoroutineInactive) => {},
    ///     unexpected => panic!("unexpected result {:?}", unexpected),
    /// }
    /// # Ok(())
    /// # }
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    pub fn resume<A, R>(&self, args: A) -> Result<R>
    where
        A: ToLuaMulti<'lua>,
        R: FromLuaMulti<'lua>,
    {
        let lua = self.0.lua;
        unsafe {
            stack_err_guard(lua.state, 0, || {
                check_stack(lua.state, 1);

                lua.push_ref(lua.state, &self.0);
                let thread_state = ffi::lua_tothread(lua.state, -1);

                let status = ffi::lua_status(thread_state);
                if status != ffi::LUA_YIELD && ffi::lua_gettop(thread_state) == 0 {
                    return Err(Error::CoroutineInactive);
                }

                ffi::lua_pop(lua.state, 1);

                let args = args.to_lua_multi(lua)?;
                let nargs = args.len() as c_int;
                check_stack(thread_state, nargs);

                for arg in args {
                    lua.push_value(thread_state, arg);
                }

                let ret = ffi::lua_resume(thread_state, lua.state, nargs);
                if ret != ffi::LUA_OK && ret != ffi::LUA_YIELD {
                    error_traceback(thread_state);
                    return Err(pop_error(thread_state, ret));
                }

                let nresults = ffi::lua_gettop(thread_state);
                let mut results = MultiValue::new();
                check_stack(thread_state, 1);
                for _ in 0..nresults {
                    results.push_front(lua.pop_value(thread_state));
                }
                R::from_lua_multi(results, lua)
            })
        }
    }

    /// Gets the status of the thread.
    pub fn status(&self) -> ThreadStatus {
        let lua = self.0.lua;
        unsafe {
            stack_guard(lua.state, 0, || {
                check_stack(lua.state, 1);

                lua.push_ref(lua.state, &self.0);
                let thread_state = ffi::lua_tothread(lua.state, -1);
                ffi::lua_pop(lua.state, 1);

                let status = ffi::lua_status(thread_state);
                if status != ffi::LUA_OK && status != ffi::LUA_YIELD {
                    ThreadStatus::Error
                } else if status == ffi::LUA_YIELD || ffi::lua_gettop(thread_state) > 0 {
                    ThreadStatus::Resumable
                } else {
                    ThreadStatus::Unresumable
                }
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Thread, ThreadStatus};
    use error::Error;
    use function::Function;
    use lua::Lua;

    #[test]
    fn test_thread() {
        let lua = Lua::new();
        let thread = lua.create_thread(
            lua.eval::<Function>(
                r#"
                    function (s)
                        local sum = s
                        for i = 1,4 do
                            sum = sum + coroutine.yield(sum)
                        end
                        return sum
                    end
                "#,
                None,
            ).unwrap(),
        ).unwrap();

        assert_eq!(thread.status(), ThreadStatus::Resumable);
        assert_eq!(thread.resume::<_, i64>(0).unwrap(), 0);
        assert_eq!(thread.status(), ThreadStatus::Resumable);
        assert_eq!(thread.resume::<_, i64>(1).unwrap(), 1);
        assert_eq!(thread.status(), ThreadStatus::Resumable);
        assert_eq!(thread.resume::<_, i64>(2).unwrap(), 3);
        assert_eq!(thread.status(), ThreadStatus::Resumable);
        assert_eq!(thread.resume::<_, i64>(3).unwrap(), 6);
        assert_eq!(thread.status(), ThreadStatus::Resumable);
        assert_eq!(thread.resume::<_, i64>(4).unwrap(), 10);
        assert_eq!(thread.status(), ThreadStatus::Unresumable);

        let accumulate = lua.create_thread(
            lua.eval::<Function>(
                r#"
                    function (sum)
                        while true do
                            sum = sum + coroutine.yield(sum)
                        end
                    end
                "#,
                None,
            ).unwrap(),
        ).unwrap();

        for i in 0..4 {
            accumulate.resume::<_, ()>(i).unwrap();
        }
        assert_eq!(accumulate.resume::<_, i64>(4).unwrap(), 10);
        assert_eq!(accumulate.status(), ThreadStatus::Resumable);
        assert!(accumulate.resume::<_, ()>("error").is_err());
        assert_eq!(accumulate.status(), ThreadStatus::Error);

        let thread = lua.eval::<Thread>(
            r#"
                coroutine.create(function ()
                    while true do
                        coroutine.yield(42)
                    end
                end)
            "#,
            None,
        ).unwrap();
        assert_eq!(thread.status(), ThreadStatus::Resumable);
        assert_eq!(thread.resume::<_, i64>(()).unwrap(), 42);

        let thread: Thread = lua.eval(
            r#"
                coroutine.create(function(arg)
                    assert(arg == 42)
                    local yieldarg = coroutine.yield(123)
                    assert(yieldarg == 43)
                    return 987
                end)
            "#,
            None,
        ).unwrap();

        assert_eq!(thread.resume::<_, u32>(42).unwrap(), 123);
        assert_eq!(thread.resume::<_, u32>(43).unwrap(), 987);

        match thread.resume::<_, u32>(()) {
            Err(Error::CoroutineInactive) => {}
            Err(_) => panic!("resuming dead coroutine error is not CoroutineInactive kind"),
            _ => panic!("resuming dead coroutine did not return error"),
        }
    }

    #[test]
    fn coroutine_from_closure() {
        let lua = Lua::new();
        let thrd_main = lua.create_function(|_, ()| Ok(())).unwrap();
        lua.globals().set("main", thrd_main).unwrap();
        let thrd: Thread = lua.eval("coroutine.create(main)", None).unwrap();
        thrd.resume::<_, ()>(()).unwrap();
    }

    #[test]
    #[should_panic]
    fn coroutine_panic() {
        let lua = Lua::new();
        let thrd_main = lua.create_function(|lua, ()| {
            // whoops, 'main' has a wrong type
            let _coro: u32 = lua.globals().get("main").unwrap();
            Ok(())
        }).unwrap();
        lua.globals().set("main", thrd_main.clone()).unwrap();
        let thrd: Thread = lua.create_thread(thrd_main).unwrap();
        thrd.resume::<_, ()>(()).unwrap();
    }
}
