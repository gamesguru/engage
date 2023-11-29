//! xtask

use clap::Parser;

mod args;
mod print_readme;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = args::Args::parse();

    match args {
        args::Args::PrintReadme => print_readme::main(),
    }
}
