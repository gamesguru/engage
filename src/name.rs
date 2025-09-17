//! Utilities for naming values.

use std::fmt;

/// A value with an associated name.
///
/// The [`Display`](fmt::Display) `impl` of this type prints only the `name`
/// field.
#[derive(Debug, Clone)]
pub(crate) struct Named<T> {
    /// The name.
    pub(crate) name: String,

    /// The value.
    pub(crate) value: T,
}

impl<T> fmt::Display for Named<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.name.fmt(f)
    }
}
