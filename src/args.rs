//! Command line arguments

use clap::Parser;

/// Yet another task runner
#[derive(Parser)]
#[clap(author, version, about)]
pub struct Args {
    /// Output Graphviz `dot` representation of the task/group DAG and exit
    #[clap(long)]
    pub dot: bool,
}
