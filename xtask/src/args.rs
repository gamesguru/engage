//! Command line arguments.

use clap::Parser;

/// Command line arguments.
#[derive(Parser)]
pub(crate) enum Args {
    /// Update the readme.
    ///
    /// Prints the new contents to `stdout`.
    PrintReadme,
}
