//! Command line arguments

use clap::Parser;

/// A task runner with DAG-based parallelism
#[derive(Parser)]
#[clap(author, version, about)]
pub struct Args {
    /// Available subcommands
    ///
    /// If unspecified, all groups and tasks in the Engage file are run.
    #[clap(subcommand)]
    pub subcmd: Option<Subcommand>,
}

/// Top-level subcommands
#[derive(clap::Subcommand)]
pub enum Subcommand {
    /// Run a built-in command
    #[clap(subcommand, name = "self")]
    Builtin(Builtin),

    /// Run a specific group or task
    Just(Just),
}

/// Built-in commands
#[derive(clap::Subcommand)]
pub enum Builtin {
    /// Output Graphviz' `dot` representation of the task/group DAG and exit
    Dot,
}

/// A specific group or task to run
#[derive(clap::Args)]
pub struct Just {
    /// A specific group of tasks to run
    ///
    /// Does not run any of the dependencies of this group
    pub group: String,

    /// A specific task in the group to run, if any
    ///
    /// If specified, the dependencies of the chosen task will not be run, only
    /// the chosen task itself.
    pub task: Option<String>,
}
