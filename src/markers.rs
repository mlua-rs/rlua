use std::cell::{Cell, UnsafeCell};
use std::marker::PhantomData;

pub type NoRefUnwindSafe = PhantomData<UnsafeCell<()>>;
pub type NoUnwindSafe = PhantomData<*mut NoRefUnwindSafe>;
pub type Invariant<'a> = PhantomData<Cell<&'a ()>>;
