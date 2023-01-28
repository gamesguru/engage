#![warn(missing_docs)]
#![warn(clippy::pedantic)]
#![warn(clippy::as_conversions)]
#![warn(clippy::dbg_macro)]
#![warn(clippy::empty_structs_with_brackets)]
#![warn(clippy::get_unwrap)]
#![warn(clippy::if_then_some_else_none)]
#![warn(clippy::let_underscore_must_use)]
#![warn(clippy::map_err_ignore)]
#![warn(clippy::missing_docs_in_private_items)]
#![warn(clippy::negative_feature_names)]
#![warn(clippy::rc_buffer)]
#![warn(clippy::rc_mutex)]
#![warn(clippy::redundant_feature_names)]
#![warn(clippy::rest_pat_in_fully_bound_structs)]
#![warn(clippy::str_to_string)]
#![warn(clippy::string_add)]
#![warn(clippy::string_slice)]
#![warn(clippy::string_to_string)]
#![warn(clippy::undocumented_unsafe_blocks)]
#![warn(clippy::unneeded_field_pattern)]
#![warn(clippy::unseparated_literal_suffix)]
#![warn(clippy::unwrap_used)]
#![warn(clippy::wildcard_dependencies)]

//! # `engage`
#![doc = "\n"]
#![doc = include_str!("../assets/tagline.txt")]

use std::{
    env,
    io::{stdout, Write},
};

use crossterm::style::{Attribute, SetAttribute, Stylize};
use petgraph::dot::Dot;

mod args;
mod error;
mod file;
mod graph;
mod ui;

#[tokio::main]
async fn main() {
    match try_main().await {
        Ok(()) => (),
        Err(e) => {
            if let error::Main::RunGraph(error::RunGraph::Task {
                source,
                task,
                longest_prefix,
            }) = &e
            {
                // This goes to `stdout` because it's information the user will
                // pretty much always want to see
                println!(
                    "{}{} {e}{c} {n} failed: {r}",
                    SetAttribute(Attribute::Reset),
                    ui::fmt_sequence(ui::Sequence::End, *longest_prefix).blue(),
                    e = "failure".bold().red(),
                    c = ':'.bold(),
                    n = ui::names_to_prefix(&task.group, &task.name),
                    r = error::Chain(source),
                );

                if let error::Task::ExitStatus(e) = source {
                    // Try to exit with the same status code as the failed
                    // command
                    std::process::exit(e.code().unwrap_or(1));
                } else {
                    std::process::exit(1);
                }
            } else {
                // Something unusual failed, report it and error out
                eprintln!(
                    "{}{} {}",
                    "error".bold().red(),
                    ':'.bold(),
                    error::Chain(&e),
                );
                std::process::exit(1);
            }
        }
    }
}

/// Fallible version of [`main`](main)
async fn try_main() -> Result<(), error::Main> {
    use error::Main as Error;

    let args = args::parse();

    // Find the Engage file and change the current directory to its directory
    let file = match args.file {
        None => file::find().await.map_err(Error::FileFind)?,
        Some(file) => file.canonicalize().map_err(Error::CanonicalizeGiven)?,
    };
    env::set_current_dir(file.parent().ok_or(Error::NoParentDirectory)?)
        .map_err(Error::ChangeDirectory)?;

    let contents = std::fs::read_to_string(file).map_err(Error::ReadFile)?;

    let mut file: file::File = toml::from_str(&contents)?;

    file.normalize();
    file.validate()?;

    match args.subcmd {
        // Run everything
        None => {
            let graph = graph::from_file(&file)?;
            ui::run_graph(graph, file).await.map_err(Into::into)
        }

        // Run a subgraph
        Some(args::Subcommand::Just(args::Just {
            group,
            task,
        })) => {
            let graph = graph::subgraph_targeting(
                &graph::from_file(&file).map_err(Error::Graph)?,
                group,
                task,
            )
            .map_err(Error::NotFound)?;

            ui::run_graph(graph, file).await.map_err(Into::into)
        }

        // Show the Graphviz' `dot` representation of the selection of the graph
        Some(args::Subcommand::Builtin(args::Builtin::Dot {
            group,
            task,
        })) => {
            let graph = graph::from_file(&file)?;

            let graph = match group {
                None => graph,
                Some(group) => graph::subgraph_targeting(&graph, group, task)?,
            };

            let x = Dot::new(&graph);

            print!("{}", x);

            // Just in case
            stdout().lock().flush().map_err(Error::Stdout)?;

            Ok(())
        }

        // List available groups and tasks
        Some(args::Subcommand::Builtin(args::Builtin::List)) => {
            // Unstable is fine because duplicate names are not allowed
            file.groups.sort_unstable_by(|a, b| a.name.cmp(&b.name));
            file.tasks.sort_unstable_by(|a, b| a.name.cmp(&b.name));

            for (i, group) in file.groups.iter().enumerate() {
                println!("{}:", group.name);

                let tasks = file.tasks.iter().filter(|x| x.group == group.name);

                for task in tasks {
                    println!("    {}", task.name);
                }

                if i + 1 < file.groups.len() {
                    println!();
                }
            }

            Ok(())
        }
    }
}
