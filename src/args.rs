//! Command line arguments

use std::path::PathBuf;

use clap::Parser;

/// A task runner with DAG-based parallelism
///
/// * All task scripts are executed with the working directory set to the
///   location of the Engage file.
///
/// * Subcommands that require the Engage file can be executed from any
///   directory so long as either the current directory or any of its ancestors
///   contain the Engage file.
///
/// * Group and task dependencies must form a directed acyclic graph; Engage
///   will enforce this. In other words, dependency cycles are not allowed.
///
/// * If a task fails, any subsequent tasks will not be executed and Engage will
///   exit with the same value as the failed task.
///
/// * If no subcommand is supplied, all groups and tasks will be scheduled based
///   on their dependencies and executed appropriately.
#[derive(Parser)]
#[clap(author, version, about)]
pub struct Args {
    /// Manually choose the Engage file
    ///
    /// This overrides the default searching behavior. Tasks will still be
    /// executed with the parent directory of the chosen file as their current
    /// working directory.
    #[clap(short, long)]
    pub file: Option<PathBuf>,

    /// Available subcommands
    ///
    /// If `None`, all groups and tasks in the Engage file are run. This doc
    /// comment does not appear in help messages.
    #[clap(subcommand)]
    pub subcmd: Option<Subcommand>,
}

/// Top-level subcommands
///
/// This doc comment does not appear in help messages.
#[derive(clap::Subcommand)]
pub enum Subcommand {
    /// Run a built-in command
    #[clap(subcommand, name = "self")]
    Builtin(Builtin),

    /// Run a specific group or task
    ///
    /// Use `engage self dot <GROUP> [TASK]` to see what exactly would be run
    /// when the same arguments are provided to this subcommand.
    Just(Just),
}

/// Built-in commands
///
/// This doc comment does not appear in help messages.
#[derive(clap::Subcommand)]
pub enum Builtin {
    /// Output Graphviz' `dot` representation of the DAG and exit
    ///
    /// Without any arguments, the DAG of the entire Engage file will be shown.
    /// This is a good way to see what `engage just <GROUP> [TASK]` would do.
    Dot {
        /// Select a specific group to show the DAG for
        group: Option<String>,

        /// Select a specific task to show the DAG for
        task: Option<String>,
    },

    /// Print a list of the available groups and tasks
    List,
}

/// A specific group or task to run
#[derive(clap::Args)]
pub struct Just {
    /// The group to run
    ///
    /// All the dependencies of the group will be executed before the chosen
    /// group is run, as usual.
    pub group: String,

    /// The task in `<GROUP>`` to run
    ///
    /// All the dependencies of the task will be executed before the chosen
    /// task is run, including dependencies of the group it belongs to, as
    /// usual.
    pub task: Option<String>,
}
