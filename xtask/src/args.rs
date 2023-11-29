//! Command line arguments

use clap::Parser;

#[allow(clippy::missing_docs_in_private_items)]
#[derive(Parser)]
pub(crate) enum Args {
    /// Update the readme
    ///
    /// Prints the new contents to `stdout`.
    PrintReadme,
}
