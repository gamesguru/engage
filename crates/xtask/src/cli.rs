//! Command line interface.

use clap::Parser;

/// Command line interface.
#[derive(Parser)]
#[command(version, about)]
pub(crate) enum Cli {
    /// Run tests.
    Test,

    /// Run clippy.
    Clippy,

    /// Run rustdoc.
    Rustdoc,
}
