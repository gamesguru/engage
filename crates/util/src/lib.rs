#![doc = env!("CARGO_PKG_DESCRIPTION")]

use std::{
    cell::UnsafeCell,
    mem::ManuallyDrop,
    ops::{Deref, DerefMut},
};

use nix::{libc::pid_t, unistd::Pid};
use tokio::process::Child;

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

/// Extension trait for [`Child`].
pub trait ChildExt {
    /// Get the [`Pid`] of the child if it is running.
    fn pid(&self) -> Option<Pid>;
}

impl ChildExt for Child {
    fn pid(&self) -> Option<Pid> {
        self.id().map(|x| {
            #[expect(
                clippy::cast_possible_wrap,
                reason = "id is stored as pid_t internally, then cast to u32, \
                          so this just casts it back"
            )]
            Pid::from_raw(x as pid_t)
        })
    }
}

/// [`UnsafeCell`] except also [`Sync`].
// TODO: Replace with the `sync_unsafe_cell` feature when it's stabilized.
#[repr(transparent)]
pub struct SyncUnsafeCell<T: ?Sized>(UnsafeCell<T>);

impl<T> SyncUnsafeCell<T> {
    /// Create a new [`SyncUnsafeCell`].
    pub fn new(value: T) -> Self {
        Self(UnsafeCell::new(value))
    }

    /// Get a mutable pointer to the inner value.
    pub fn get(&self) -> *mut T {
        self.0.get()
    }
}

// SAFETY: It's up to the user to ensure access is synchronized.
unsafe impl<T> Sync for SyncUnsafeCell<T> where T: Sync + ?Sized {}

impl<T> Default for SyncUnsafeCell<T>
where
    T: Default,
{
    fn default() -> Self {
        Self::new(T::default())
    }
}
