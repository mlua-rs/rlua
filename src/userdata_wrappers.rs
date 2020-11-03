//! Implements UserData for a number of helpful Rust std types.

use std::{
    mem, ptr,
    sync::{Arc, Mutex},
};

use crate::{
    ffi, Context, Function, MetaMethod, MultiValue, Result, Table, UserData, UserDataMethods, Value,
};

const ALL_METAMETHOD_KEYS: &[MetaMethod] = &[
    MetaMethod::Add,
    MetaMethod::BAnd,
    MetaMethod::BAnd,
    MetaMethod::BNot,
    MetaMethod::BOr,
    MetaMethod::BXor,
    MetaMethod::Call,
    MetaMethod::Concat,
    MetaMethod::Div,
    MetaMethod::Eq,
    MetaMethod::IDiv,
    MetaMethod::Index,
    MetaMethod::Le,
    MetaMethod::Len,
    MetaMethod::Lt,
    MetaMethod::Mod,
    MetaMethod::Mul,
    MetaMethod::NewIndex,
    MetaMethod::Pairs,
    MetaMethod::Pow,
    MetaMethod::Shl,
    MetaMethod::Shr,
    MetaMethod::Sub,
    MetaMethod::ToString,
    MetaMethod::Unm,
];

/// `Arc<Mutex<T>>` will act more or less like the original `T`.
/// It does this by registering metamethods that, when called, just act on the original `T`.
///
/// The `Default` trait bound is currently required, but may not be required in the future.
/// It's only use is to prevent a double-drop error.
/// The default `#[derive(Default)]` implementation should be enough.
///
/// See the source code for more details.
impl<T: 'static + Send + UserData + Default> UserData for Arc<Mutex<T>> {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        // This must implement all the same metamethods as T does.
        for mm_key in ALL_METAMETHOD_KEYS.iter() {
            methods.add_meta_method(
                *mm_key,
                move |ctx, this, args: MultiValue| -> Result<MultiValue> {
                    unsafe {
                        // Get the ID of T's metatable
                        let mt_id = ctx.userdata_metatable::<T>()?;
                        // Push the (hopefully) metatable onto the stack
                        let pushed_type = ffi::lua_rawgeti(
                            ctx.state,
                            ffi::LUA_REGISTRYINDEX,
                            mt_id as ffi::lua_Integer,
                        );
                        assert_eq!(pushed_type, ffi::LUA_TTABLE);

                        // Pop the metatable off the stack
                        let metatable = Table(ctx.pop_ref());

                        // Let's go call that metamethod
                        let method: Function =
                            metatable.raw_get(std::str::from_utf8(mm_key.name()).unwrap())?;
                        // Copy the T out of the mutex bitwise.
                        // This is so the metamethod call can mutate the T,
                        // and it's safely written back at the end.
                        let mut guard = this.lock().unwrap();
                        // Entering the NO PANIC ZONE
                        let tmp: T = ptr::read(&*guard as *const T);
                        let tmp_as_userdata = ctx.create_userdata(tmp)?;
                        // The clone here is sound as AnyUserData just holds a reference
                        let all_args = (tmp_as_userdata.clone(), args);
                        let call_res = method.call(all_args)?;
                        // the function call might have mutated the `this` value...
                        // let's get it.
                        // recover the address of the userdata out of the stack
                        let mut tmp_borrow = tmp_as_userdata.borrow_mut::<T>()?;
                        // We can't let `tmp_as_userdata` keep existing, because when
                        // it is dropped, it will also drop the original T.
                        // So we fill it with a default.
                        let recovered_tmp = mem::take::<T>(&mut tmp_borrow);
                        // Write the recovered value without dropping the T in the mutex
                        ptr::write(&mut *guard as *mut T, recovered_tmp);

                        Ok(call_res)
                    }
                },
            );
        }
    }
}
