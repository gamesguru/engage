//! Command line arguments

use std::path::PathBuf;

use clap::{CommandFactory, FromArgMatches, Parser};

/// Command-line arguments
#[derive(Parser)]
#[clap(author, version)]
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

/// Parses arguments out of `std::env::args_os()`, exiting on error
///
/// Call this instead of `<Args as Parser>::parse`, this function does some
/// extra tweaking that isn't possible using the derive API.
pub fn parse() -> Args {
    let about = include_str!("../assets/tagline.txt").trim_end_matches('\n');

    let behavior = include_str!("../assets/behavior.md").trim_end_matches('\n');

    let long_about = format!("{}\n\n{}", about, behavior,);

    let mut args = Args::command().about(about).long_about(long_about);

    let res = Args::from_arg_matches_mut(&mut args.get_matches_mut())
        .map_err(|e| e.format(&mut Args::command()));

    match res {
        Err(e) => e.exit(),
        Ok(x) => x,
    }
}
