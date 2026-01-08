//! Utilities for naming values.

use std::{fmt, marker::PhantomData, str::FromStr};

use crate::error;

/// A value with an associated name.
///
/// The [`Display`](fmt::Display) `impl` of this type prints only the `name`
/// field.
#[derive(Debug, Clone)]
pub(crate) struct Named<T> {
    /// The name.
    pub(crate) name: Box<Name>,

    /// The value.
    pub(crate) value: T,
}

impl<T> fmt::Display for Named<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.name.fmt(f)
    }
}

/// A name.
#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub(crate) struct Name(str);

impl Name {
    /// Create a new, boxed, potentially invalid [`Name`].
    fn new_boxed_unchecked(x: Box<str>) -> Box<Self> {
        // SAFETY: Safe because `Name` is a `#[repr(transparent)]` wrapper over
        // `str`.
        unsafe { Box::from_raw(Box::into_raw(x) as *mut Self) }
    }

    /// Check whether a `&str` is a valid [`Name`].
    fn validate(x: &str) -> Result<(), error::ValidateName> {
        use error::ValidateName as E;

        let is_valid_start =
            |c: char| c.is_ascii_lowercase() || c.is_ascii_digit();

        let is_valid_continue = |c| is_valid_start(c) || c == '-';

        let mut char_indices = x.char_indices();

        if let Some((_, c)) = char_indices.next() {
            if !is_valid_start(c) {
                return Err(E::InvalidStart(c));
            }
        } else {
            return Err(E::Empty);
        }

        for (i, c) in char_indices {
            if !is_valid_continue(c) {
                return Err(E::InvalidContinue(i, c));
            }
        }

        Ok(())
    }
}

impl AsRef<Self> for Name {
    fn as_ref(&self) -> &Self {
        self
    }
}

impl AsRef<str> for Name {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl ToOwned for Name {
    type Owned = Box<Name>;

    fn to_owned(&self) -> Self::Owned {
        Self::new_boxed_unchecked(self.0.into())
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for Box<Name> {
    type Err = error::ValidateName;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Name::validate(s)?;
        Ok(Name::new_boxed_unchecked(s.into()))
    }
}

impl Clone for Box<Name> {
    fn clone(&self) -> Self {
        Name::new_boxed_unchecked(self.0.into())
    }
}

impl AsRef<str> for Box<Name> {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl<'a> PartialEq<&'a Name> for Box<Name> {
    fn eq(&self, other: &&'a Name) -> bool {
        PartialEq::eq(&**self, &**other)
    }
}

impl<'de> serde::de::Deserialize<'de> for Box<Name> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor<T>(PhantomData<T>);

        impl<T> serde::de::Visitor<'_> for Visitor<T>
        where
            T: FromStr,
            T::Err: fmt::Display,
        {
            type Value = T;

            fn expecting(
                &self,
                formatter: &mut fmt::Formatter<'_>,
            ) -> fmt::Result {
                formatter.write_str("a name")
            }

            fn visit_str<E>(self, v: &'_ str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                v.parse().map_err(serde::de::Error::custom)
            }
        }

        deserializer.deserialize_str(Visitor(PhantomData))
    }
}
