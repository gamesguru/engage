//! Command line arguments

use clap::Parser;

/// A task runner with DAG-based parallelism
#[derive(Parser)]
#[clap(author, version, about)]
pub struct Args {
    /// Output Graphviz `dot` representation of the task/group DAG and exit
    #[clap(long)]
    pub dot: bool,
}
