//! Implements UserData for a number of helpful Rust std types.

use std::{
    ptr,
    sync::{Arc, Mutex},
};

use crate::{
    ffi,
    util::{assert_stack, StackGuard},
    Function, MetaMethod, MultiValue, Table, UserData, UserDataMethods,
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
impl<T: 'static + Send + UserData> UserData for Arc<Mutex<T>> {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        // This must implement all the same metamethods as T does.
        for mm_key in ALL_METAMETHOD_KEYS.iter() {
            methods.add_meta_method(*mm_key, move |ctx, this, args: MultiValue| {
                // This code copied from Table::get_metatable
                unsafe {
                    if let Some(metatable) = {
                        // no clue what this does?
                        let _sg = StackGuard::new(ctx.state);
                        assert_stack(ctx.state, 1);

                        // What I need to do is: push the pointed-to value
                        // of the mutex to Lua's stack.
                        // This will require unsafe code.
                        let guard = this.lock().unwrap();
                        // Great, this is now the only thing that can access this data.
                        // This is safe since we will fix up the mutex before we go.
                        let tmp: T = ptr::read(&*guard as *const T);
                        let tmp_as_userdata = ctx.create_userdata(tmp)?;
                        // push the lref
                        ctx.push_ref(&tmp_as_userdata.0);
                        // Try and get its metatable
                        let mt = if ffi::lua_getmetatable(ctx.state, -1) == 0 {
                            None
                        } else {
                            let table = Table(ctx.pop_ref());
                            Some(table)
                        };
                        // now put the data back in the guard
                        let tmp_borrowed = tmp_as_userdata.borrow_mut::<T>()?;
                        let tmp_recovered = ptr::read(&*tmp_borrowed as *const T);
                        // presenting the worst Rust code you've ever seen this side of winapi
                        ptr::write(&*guard as *const T as *mut T, tmp_recovered);
                        // and as `tmp_as_userdata` is dropped here, all is well.
                        mt
                    } {
                        // This userdata has a metatable!
                        // Let's go call that metamethod
                        let method: Function =
                            metatable.raw_get(std::str::from_utf8(mm_key.name()).unwrap())?;
                        // Do more unsafe shenanigans.
                        // This is so the metamethod call can mutate the T,
                        // and it's safely written back at the end.
                        let guard = this.lock().unwrap();
                        let tmp: T = ptr::read(&*guard as *const T);
                        let tmp_as_userdata = ctx.create_userdata(tmp)?;
                        // The clone here is sound as AnyUserData just holds a reference
                        let call_res = method.call((tmp_as_userdata.clone(), args))?;
                        let tmp_borrowed = tmp_as_userdata.borrow_mut::<T>()?;
                        let tmp_recovered = ptr::read(&*tmp_borrowed as *const T);
                        ptr::write(&*guard as *const T as *mut T, tmp_recovered);
                        Ok(call_res)
                    } else {
                        // There's no metatable to get a metamethod from
                        // Return nil.
                        Ok(MultiValue::new())
                    }
                }
            });
        }
    }
}
