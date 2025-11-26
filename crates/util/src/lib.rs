#![doc = env!("CARGO_PKG_DESCRIPTION")]

use std::{
    mem::ManuallyDrop,
    ops::{Deref, DerefMut},
};

/// Run a function for a value when it is dropped.
// TODO: Replace with the `drop_guard` feature when it's stabilized.
pub struct DropGuard<T, F>(ManuallyDrop<T>, ManuallyDrop<F>)
where
    F: FnOnce(T);

impl<T, F> DropGuard<T, F>
where
    F: FnOnce(T),
{
    /// Wrap `inner` so that `drop` is called when the returned value is
    /// dropped.
    pub fn new(inner: T, drop: F) -> Self {
        Self(ManuallyDrop::new(inner), ManuallyDrop::new(drop))
    }
}

impl<T, F> Drop for DropGuard<T, F>
where
    F: FnOnce(T),
{
    fn drop(&mut self) {
        // SAFETY: `self.0` is not accessed again after this.
        let inner = unsafe { ManuallyDrop::take(&mut self.0) };

        // SAFETY: `self.1` is not accessed again after this.
        let drop = unsafe { ManuallyDrop::take(&mut self.1) };

        drop(inner);
    }
}

impl<T, F> Deref for DropGuard<T, F>
where
    F: FnOnce(T),
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T, F> DerefMut for DropGuard<T, F>
where
    F: FnOnce(T),
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
