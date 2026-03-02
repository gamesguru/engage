//! Command line interface.

use std::{
    fmt::{self, Write as _},
    path::PathBuf,
    str::FromStr as _,
};

use crate::name::Name;

/// Log format.
#[derive(Copy, Clone, PartialEq, Eq, Default, clap::ValueEnum)]
pub(crate) enum LogFormat {
    /// The default format.
    #[default]
    Default,

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
            LogFormat::Default => f.write_str("default"),
            LogFormat::Pretty => f.write_str("pretty"),
            LogFormat::Full => f.write_str("full"),
            LogFormat::Compact => f.write_str("compact"),
            LogFormat::Json => f.write_str("json"),
        }
    }
}

/// Command line arguments.
pub(crate) enum Args {
    /// Run processes.
    Run {
        /// The manually-selected Engage file, if any.
        file: Option<PathBuf>,

        /// The selected process, if any.
        process: Option<Box<Name>>,

        /// The selected log format.
        log_format: LogFormat,
    },

    /// Print a graph in Graphviz' DOT language of processes and their
    /// dependencies.
    Dot {
        /// The manually-selected Engage file, if any.
        file: Option<PathBuf>,

        /// The selected process, if any.
        process: Option<Box<Name>>,
    },

    /// Print available processes.
    List {
        /// The manually-selected Engage file, if any.
        file: Option<PathBuf>,
    },

    /// Print completions for a supported shell.
    Completions {
        /// The shell to print completions for.
        shell: clap_complete::Shell,
    },
}

impl Args {
    /// Get the selected [`LogFormat`].
    pub(crate) fn log_format(&self) -> LogFormat {
        if let Self::Run {
            log_format,
            ..
        } = self
        {
            return *log_format;
        }

        LogFormat::Default
    }
}

/// Get the [`clap::Command`] that models the command line interface.
pub(crate) fn command() -> clap::Command {
    let completions = {
        let shell = clap::Arg::new("shell")
            .value_parser(
                clap::builder::EnumValueParser::<clap_complete::Shell>::new(),
            )
            .value_name("SHELL")
            .required(true)
            .help("The shell to print completions for");

        clap::Command::new("completions")
            .about("Print completions for a supported shell")
            .arg(shell)
    };

    let dot = {
        let about = "Print a graph in Graphviz' DOT language of processes and \
                     their dependencies";
        let long_about = "This command can be used to visualize what the \
                          `engage` command would do (especially since they \
                          take most of the same options) or to debug \
                          unexpected process dependencies. Notably, this \
                          command will not exit with an error if the Engage \
                          file contains dependency cycles, which makes it \
                          useful for debugging those as well.";

        clap::Command::new("dot")
            .about(about)
            .long_about(format!("{about}\n\n{long_about}"))
            .arg(arg_file())
            .arg(arg_process())
    };

    let list = clap::Command::new("list")
        .about("Print available processes")
        .arg(arg_file());

    let about = env!("CARGO_PKG_DESCRIPTION")
        .strip_suffix('.')
        .expect("CARGO_PKG_DESCRIPTION should end with a `.`");

    let long_about = "This command (without any other command) will attempt \
                      to spawn all processes in the selected Engage file, \
                      starting with processes without dependencies and \
                      spawning subsequent processes as their dependencies \
                      become ready. If specified, the `-p`/`--process` option \
                      will cause Engage to only attempt to spawn the selected \
                      process and its dependencies.";

    let mut long_about = format!("{about}\n\n{long_about}");
    if let Some(x) = option_env!("ENGAGE_DOCS_LINK") {
        write!(&mut long_about, "\n\nRead the book for more information: {x}")
            .expect("in-memory write should succeed");
    }

    clap::Command::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .about(about)
        .long_about(long_about)
        .args_conflicts_with_subcommands(true)
        .subcommand(completions)
        .subcommand(dot)
        .subcommand(list)
        .arg(arg_file())
        .arg(arg_log_format())
        .arg(arg_process())
}

/// Build the `-f`/`--file` argument.
fn arg_file() -> clap::Arg {
    let help = "Manually select the Engage file";
    let long_help = "This option overrides the default searching behavior.";

    clap::Arg::new("file")
        .value_parser(
            <PathBuf as clap::builder::ValueParserFactory>::value_parser(),
        )
        .long("file")
        .short('f')
        .value_name("FILE")
        .help(help)
        .long_help(format!("{help}\n\n{long_help}"))
}

/// Build the `-l`/`--log-format` argument.
fn arg_log_format() -> clap::Arg {
    let help = "Select the log format";
    let long_help = "The `default` format is the \"intended experience\". The \
                     other formats are primarily useful for debugging or \
                     getting structured logs. Formats other than `default` \
                     will use the value of the `RUST_LOG` environment \
                     variable as the log filter if it's set, otherwise `info` \
                     is used.";

    clap::Arg::new("log-format")
        .value_parser(clap::builder::EnumValueParser::<LogFormat>::new())
        .long("log-format")
        .short('l')
        .value_name("FORMAT")
        .default_value("default")
        .help(help)
        .long_help(format!("{help}\n\n{long_help}"))
}

/// Build the `-p`/`--process` argument.
fn arg_process() -> clap::Arg {
    let help = "Select a process";

    clap::Arg::new("process")
        .value_parser(clap::builder::ValueParser::new(Box::<Name>::from_str))
        .long("process")
        .short('p')
        .value_name("PROCESS")
        .help(help)
}

/// Parses arguments out of `std::env::args_os()`, exiting on error.
pub(crate) fn try_parse() -> Result<Args, clap::Error> {
    let mut matches = command().try_get_matches()?;

    let mut subcommand = matches.remove_subcommand();
    let subcommand = subcommand.as_mut().map(|(x, y)| (&**x, y));
    match subcommand {
        None => Ok(Args::Run {
            file: matches.remove_one("file"),
            log_format: matches
                .remove_one("log-format")
                .expect("at minimum the default should be set"),
            process: matches.remove_one("process"),
        }),

        Some(("completions", matches)) => Ok(Args::Completions {
            shell: matches
                .remove_one("shell")
                .expect("required argument should be supplied"),
        }),

        Some(("dot", matches)) => Ok(Args::Dot {
            file: matches.remove_one("file"),
            process: matches.remove_one("process"),
        }),

        Some(("list", matches)) => Ok(Args::List {
            file: matches.remove_one("file"),
        }),

        _ => unreachable!(),
    }
}
