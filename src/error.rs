//! Error handling facilities

use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    iter,
};

/// Wraps any [`Error`][e] type so that [`Display`][d] includes its sources
///
/// # Examples
///
/// If `Foo` has a source of `Bar`, and `Bar` has a source of `Baz`, then
/// the formatted output of `Chain(&Foo)` will look like this:
///
/// ```
/// # use engage::error::Chain;
/// # use thiserror::Error;
/// # #[derive(Debug, Error)]
/// # #[error("foo")]
/// # struct Foo(#[from] Bar);
/// # #[derive(Debug, Error)]
/// # #[error("bar")]
/// # struct Bar(#[from] Baz);
/// # #[derive(Debug, Error)]
/// # #[error("baz")]
/// # struct Baz;
/// # fn try_foo() -> Result<(), Foo> { Err(Foo(Bar(Baz))) }
/// match try_foo() {
///     Ok(foo) => {
///         // Do something with foo
///         # drop(foo);
///         # unreachable!()
///     }
///     Err(e) => {
///         assert_eq!(
///             format!("foo error: {}", Chain(&e)),
///             "foo error: foo: bar: baz"
///         );
///     }
/// }
/// ```
///
/// [e]: Error
/// [d]: Display
#[derive(Debug)]
pub struct Chain<'a>(pub &'a dyn Error);

impl<'a> Display for Chain<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)?;

        let mut source = self.0.source();

        source
            .into_iter()
            .chain(iter::from_fn(|| {
                source = source.and_then(Error::source);
                source
            }))
            .try_for_each(|source| write!(f, ": {}", source))
    }
}
