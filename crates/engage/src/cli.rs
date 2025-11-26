//! Command line interface.

use std::{
    fmt::{self, Write as _},
    num::NonZeroUsize,
    path::PathBuf,
};

use clap::{CommandFactory as _, FromArgMatches as _, Parser, ValueEnum};
use indoc::indoc;

use crate::name::Name;

/// Log format.
#[derive(Copy, Clone, PartialEq, Eq, Default, ValueEnum)]
pub(crate) enum LogFormat {
    /// The normal log format.
    #[default]
    Normal,

    /// [`tracing_subscriber`]'s compact format.
    Compact,

    /// [`tracing_subscriber`]'s full format.
    Full,

    /// [`tracing_subscriber`]'s pretty format.
    Pretty,

    /// [`tracing_subscriber`]'s JSON format.
    Json,
}

impl fmt::Display for LogFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogFormat::Normal => f.write_str("normal"),
            LogFormat::Pretty => f.write_str("pretty"),
            LogFormat::Full => f.write_str("full"),
            LogFormat::Compact => f.write_str("compact"),
            LogFormat::Json => f.write_str("json"),
        }
    }
}

/// Command line arguments.
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

    /// Log format.
    ///
    /// By default this uses Engage's custom log format which is
    /// optimal for regular usage. Other log formats are implemented by
    /// [`tracing_subscriber`] and also respect the `RUST_LOG` environment
    /// variable; these formats are primarily useful for debugging.
    #[clap(short, long, default_value_t = LogFormat::Normal)]
    pub(crate) log_format: LogFormat,

    /// Available subcommands.
    ///
    /// If `None`, all tasks in the Engage file are run. This doc comment does
    /// not appear in help messages.
    #[clap(subcommand)]
    pub(crate) subcmd: Option<Subcommand>,
}

/// Top-level subcommands.
///
/// This doc comment does not appear in help messages.
#[derive(clap::Subcommand)]
pub(crate) enum Subcommand {
    /// Run a specific task.
    ///
    /// Use `engage dot [TASK]` to see what exactly would be run when the same
    /// arguments are provided to this subcommand.
    Just {
        /// The task to run.
        ///
        /// All the dependencies of the task will be executed before the chosen
        /// task is run.
        task: Box<Name>,
    },

    /// Output Graphviz' `dot` representation of the DAG and exit.
    ///
    /// Without any arguments, the DAG of the entire Engage file will be shown.
    ///
    /// This subcommand is a good way to see what `engage just <TASK>` would do,
    /// debug task dependency cycles, or unexpected task dependencies.
    Dot {
        /// Select a specific task to show the DAG for.
        task: Option<Box<Name>>,
    },

    /// Print a list of the available tasks.
    List,

    /// Print completions for a given shell.
    Completions {
        /// The shell to print completions for.
        shell: clap_complete::Shell,
    },
}

/// Get the [`clap::Command`] that models the command line interface.
pub(crate) fn command() -> clap::Command {
    let about = env!("CARGO_PKG_DESCRIPTION");

    let long_about_body = indoc! {"
        * All task commands are executed with the working directory set to the \
          location of the Engage file.

        * Operations that require the Engage file can be invoked from the \
          directory it's in or any of that directory's children.

        * Task dependencies must form a directed acyclic graph. In other \
          words, dependency cycles are not allowed.

        * If a task fails, any dependent tasks will not be executed and Engage \
          will exit with a status of `1`.

        * If some other error occurs (e.g. configuration error), Engage will \
          exit with a status of `2`.

        * If no subcommand is supplied, all tasks will be scheduled and \
          executed based on their dependencies."
    };

    let mut long_about = format!("{about}\n\n{long_about_body}");

    if let Some(x) = option_env!("ENGAGE_DOCS_LINK") {
        write!(
            &mut long_about,
            "\n\nFurther documentation is available at <{x}>"
        )
        .expect("in-memory write should succeed");
    }

    Args::command().about(about).long_about(long_about)
}

/// Parses arguments out of `std::env::args_os()`, exiting on error.
///
/// Call this instead of `<Args as Parser>::parse`, this function does some
/// extra tweaking that isn't possible using the derive API.
pub(crate) fn try_parse() -> Result<Args, clap::Error> {
    let mut command = command();
    Args::from_arg_matches(&command.get_matches_mut())
}
