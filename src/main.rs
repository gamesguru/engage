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

//! The binary

use std::{
    env,
    error::Error as StdError,
    io::{stdout, Write},
    ops::ControlFlow,
    sync::Arc,
};

use clap::Parser;
use engage::{
    args::{Args, Builtin, Just, Subcommand},
    error, find_file, graph, File,
};
use petgraph::{
    dot::Dot,
    graph::{DefaultIx, DiGraph},
};

#[tokio::main]
async fn main() {
    let args = Args::parse();

    match try_main(args).await {
        Ok(()) => (),
        Err(e) => {
            if let Some(error::Task::ExitStatus(e)) =
                e.downcast_ref::<error::Task>()
            {
                // Try to exit with the same status code as the failed command
                std::process::exit(e.code().unwrap_or(1));
            } else {
                // Something unusual failed, report it and error out
                eprint!("{}", error::format_cli(error::Chain(&*e)));
                std::process::exit(1);
            }
        }
    }
}

/// Fallible version of [`main`](main)
async fn try_main(args: Args) -> Result<(), Box<dyn StdError>> {
    // Find the Engage file and change the current directory to its directory
    let file = match args.file {
        None => find_file().await?,
        Some(file) => file.canonicalize()?,
    };
    env::set_current_dir(file.parent().ok_or_else(|| {
        Box::<dyn StdError>::from("path to file has no parent directory")
    })?)?;

    let contents = std::fs::read_to_string(file)?;
    let mut file: File = toml::from_str(&contents)?;
    file.update_groups();
    file.validate()?;

    match args.subcmd {
        // Run everything
        None => {
            let graph = graph::from_file(&file)?;
            run_all(file, graph).await
        }

        // Run a subgraph
        Some(Subcommand::Just(Just {
            group,
            task,
        })) => {
            let graph = graph::subgraph_targeting(
                &graph::from_file(&file)?,
                group,
                task,
            )?;

            run_all(file, graph).await
        }

        // Show the Graphviz' `dot` representation of the selection of the graph
        Some(Subcommand::Builtin(Builtin::Dot {
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
            stdout().lock().flush()?;

            Ok(())
        }

        // List available groups and tasks
        Some(Subcommand::Builtin(Builtin::List)) => {
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

/// Run all groups and tasks in the given `Engage` object
async fn run_all(
    file: File,
    graph: DiGraph<graph::Node, u32, DefaultIx>,
) -> Result<(), Box<dyn StdError>> {
    graph::ensure_acyclic(&graph)?;
    let file = Arc::new(file);

    graph::execute(Arc::new(graph), move |node| {
        let file = file.clone();
        async move {
            if let graph::Node::Task(task) = node {
                if let Err(e) = file.run_task(Arc::new(task)).await {
                    return ControlFlow::Break(e);
                }
            }

            ControlFlow::Continue(())
        }
    })
    .await
    .map_or_else(|| Ok(()), |e| Err(e.into()))
}
