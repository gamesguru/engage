//! Errors.

// Allowed because this kind of information should be encapsulated in the error
// messages anyway.
#![allow(clippy::allow_attributes, clippy::missing_docs_in_private_items)]

use derail::CoreCompat;
use derail_macros::Error;

/// Top-level errors.
#[derive(Debug, Error)]
#[derail(type Details = ())]
pub(crate) enum Main {
    Test(#[derail(skip_self)] CargoCommand),
    Clippy(#[derail(skip_self)] CargoCommand),
    Rustdoc(#[derail(skip_self)] CargoCommand),
}

/// Errors running a subprocess that require cargo metadata to spawn.
#[derive(Debug, Error)]
#[derail(type Details = ())]
pub(crate) enum CargoCommand {
    #[derail(display("failed to get cargo metadata"))]
    GetMetadata(#[derail(child)] CoreCompat<cargo_metadata::Error>),

    #[derail(display("failed to spawn process"))]
    Spawn(#[derail(child)] CoreCompat<std::io::Error>),

    #[derail(display("failed to wait for process"))]
    Wait(#[derail(child)] CoreCompat<std::io::Error>),

    #[derail(display("process for package {_0} with features {_1:?} failed"))]
    Failure(String, Vec<String>),
}
