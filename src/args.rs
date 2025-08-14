//! Command line arguments.

use std::{num::NonZeroUsize, path::PathBuf};

use clap::{CommandFactory as _, FromArgMatches as _, Parser};
use indoc::indoc;

/// Command-line arguments.
#[derive(Parser)]
#[clap(version)]
pub(crate) struct Args {
    /// Manually choose the Engage file.
    ///
    /// This overrides the default searching behavior. Tasks will still be
    /// executed with the parent directory of the chosen file as their current
    /// working directory.
    #[clap(short, long)]
    pub(crate) file: Option<PathBuf>,

    /// Maximum amount of tasks to run at once.
    ///
    /// Not specifying this option results in the default, which is no limit.
    /// The mimimum valid value is `1`.
    #[clap(short, long)]
    pub(crate) jobs: Option<NonZeroUsize>,

    /// Available subcommands.
    ///
    /// If `None`, all groups and tasks in the Engage file are run. This doc
    /// comment does not appear in help messages.
    #[clap(subcommand)]
    pub(crate) subcmd: Option<Subcommand>,
}

/// Top-level subcommands.
///
/// This doc comment does not appear in help messages.
#[derive(clap::Subcommand)]
pub(crate) enum Subcommand {
    /// Run a specific group or task.
    ///
    /// Use `engage dot <GROUP> [TASK]` to see what exactly would be run when
    /// the same arguments are provided to this subcommand.
    Just(Just),

    /// Output Graphviz' `dot` representation of the DAG and exit.
    ///
    /// Without any arguments, the DAG of the entire Engage file will be shown.
    /// This is a good way to see what `engage just <GROUP> [TASK]` would do.
    Dot {
        /// Select a specific group to show the DAG for.
        group: Option<String>,

        /// Select a specific task to show the DAG for.
        task: Option<String>,
    },

    /// Print a list of the available groups and tasks.
    List,

    /// Print a JSON Schema document describing the format of an Engage file.
    Schema,

    /// Print completions for a given shell.
    Completions {
        /// The shell to print completions for.
        shell: clap_complete::Shell,
    },
}

/// A specific group or task to run.
#[derive(clap::Args)]
pub(crate) struct Just {
    /// The group to run.
    ///
    /// All the dependencies of the group will be executed before the chosen
    /// group is run, as usual.
    pub(crate) group: String,

    /// The task in that group to run.
    ///
    /// All the dependencies of the task will be executed before the chosen
    /// task is run, including dependencies of the group it belongs to, as
    /// usual.
    pub(crate) task: Option<String>,
}

/// Get the [`clap::Command`] that models the command line interface.
pub(crate) fn command() -> clap::Command {
    let about = env!("CARGO_PKG_DESCRIPTION");

    let long_about_body = indoc! {"
        * All task scripts are executed with the working directory set to the \
          location of the Engage file.

        * Operations that require the Engage file can be invoked from the \
          directory it's in or any of that directory's children.

        * Group and task dependencies must form a directed acyclic graph. In \
          other words, dependency cycles are not allowed.

        * If a task fails, any dependent tasks will not be executed and Engage \
          will exit with a status of `1`.

        * If some other error occurs (e.g. configuration error), Engage will \
          exit with a status of `2`.

        * If no subcommand is supplied, all groups and tasks will be scheduled \
          and executed based on their dependencies."
    };

    let long_about = format!("{about}\n\n{long_about_body}");

    Args::command().about(about).long_about(long_about)
}

/// Parses arguments out of `std::env::args_os()`, exiting on error.
///
/// Call this instead of `<Args as Parser>::parse`, this function does some
/// extra tweaking that isn't possible using the derive API.
pub(crate) fn parse() -> Args {
    let mut command = command();

    let res = Args::from_arg_matches(&command.get_matches_mut())
        .map_err(|e| e.format(&mut command));

    match res {
        Err(e) => e.exit(),
        Ok(x) => x,
    }
}
