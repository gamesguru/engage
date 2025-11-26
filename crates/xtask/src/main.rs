#![doc = env!("CARGO_PKG_DESCRIPTION")]

use std::{
    iter,
    process::{Command, ExitCode},
};

use cargo_metadata::{MetadataCommand, Package, Target};
use clap::Parser as _;
use cli::Cli;
use itertools::Itertools as _;

mod cli;
mod error;

fn main() -> ExitCode {
    let Err(e) = try_main() else {
        return ExitCode::SUCCESS;
    };

    let report = derail_report::multiline::<_, _, derail_report::HeapFactory>(
        iter::once(&e),
    );

    println!("Errors:\n{report}");

    ExitCode::FAILURE
}

/// Fallible entrypoint.
fn try_main() -> Result<(), error::Main> {
    let args = Cli::parse();

    match args {
        Cli::Test => {
            xtask_test().map_err(error::Main::Test)?;
        }
        Cli::Clippy => {
            xtask_clippy().map_err(error::Main::Clippy)?;
        }
        Cli::Rustdoc => {
            xtask_rustdoc().map_err(error::Main::Rustdoc)?;
        }
    }

    Ok(())
}

/// Iterate over packages in the workspace and their possible feature
/// combinations.
fn workspace_packages_and_features(
    meta: &cargo_metadata::Metadata,
) -> impl Iterator<Item = (Package, Vec<String>)> {
    meta.workspace_packages()
        .into_iter()
        .flat_map(|x| {
            iter::repeat_with(|| x.clone()).zip(
                x.features
                    .keys()
                    .map(Clone::clone)
                    .filter(|x| *x != "default")
                    .powerset(),
            )
        })
        .inspect(|(package, features)| {
            println!(
                "Checking crate {:?} with features {features:?}",
                package.name
            );
        })
}

/// Returns `true` if `package` has a `lib` target.
fn package_has_lib(package: &Package) -> bool {
    package.targets.iter().any(Target::is_lib)
}

/// Entrypoint for the `test` subcommand.
fn xtask_test() -> Result<(), error::CargoCommand> {
    let meta = MetadataCommand::new()
        .exec()
        .map_err(|x| error::CargoCommand::GetMetadata(x.into()))?;

    for (package, features) in workspace_packages_and_features(&meta) {
        let mut cmd = Command::new("cargo");

        cmd.args([
            "llvm-cov",
            "--no-cfg-coverage",
            "--all-targets",
            "--color",
            "always",
            "--no-default-features",
            "--features",
            &features.iter().join(","),
            "--package",
            &package.name,
            "--",
            "--color",
            "always",
        ]);

        // Ensure committed snapshot metadata is up to date.
        cmd.env("INSTA_REQUIRE_FULL_MATCH", "1");

        let status = cmd
            .spawn()
            .map_err(|x| error::CargoCommand::Spawn(x.into()))?
            .wait()
            .map_err(|x| error::CargoCommand::Wait(x.into()))?;

        if !status.success() {
            return Err(error::CargoCommand::Failure(
                package.name.into_inner(),
                features,
            ));
        }

        if package_has_lib(&package) {
            // `cargo-llvm-cov` doesn't currently handle doctests, so they need
            // to be run separately.
            let mut cmd = Command::new("cargo");

            cmd.args([
                "test",
                "--doc",
                "--color",
                "always",
                "--no-default-features",
                "--features",
                &features.iter().join(","),
                "--package",
                &package.name,
                "--",
                "--color",
                "always",
            ]);

            let status = cmd
                .spawn()
                .map_err(|x| error::CargoCommand::Spawn(x.into()))?
                .wait()
                .map_err(|x| error::CargoCommand::Wait(x.into()))?;

            if !status.success() {
                return Err(error::CargoCommand::Failure(
                    package.name.into_inner(),
                    features,
                ));
            }
        }
    }

    Ok(())
}

/// Entrypoint for the `clippy` subcommand.
fn xtask_clippy() -> Result<(), error::CargoCommand> {
    let meta = MetadataCommand::new()
        .exec()
        .map_err(|x| error::CargoCommand::GetMetadata(x.into()))?;

    for (package, features) in workspace_packages_and_features(&meta) {
        let mut cmd = Command::new("cargo");

        cmd.args([
            "clippy",
            "--all-targets",
            "--color",
            "always",
            "--no-default-features",
            "--features",
            &features.iter().join(","),
            "--package",
            &package.name,
            "--",
            "--deny",
            "warnings",
        ]);

        let status = cmd
            .spawn()
            .map_err(|x| error::CargoCommand::Spawn(x.into()))?
            .wait()
            .map_err(|x| error::CargoCommand::Wait(x.into()))?;

        if !status.success() {
            return Err(error::CargoCommand::Failure(
                package.name.into_inner(),
                features,
            ));
        }
    }

    Ok(())
}

/// Entrypoint for the `rustdoc` subcommand.
fn xtask_rustdoc() -> Result<(), error::CargoCommand> {
    let meta = MetadataCommand::new()
        .exec()
        .map_err(|x| error::CargoCommand::GetMetadata(x.into()))?;

    for (package, features) in workspace_packages_and_features(&meta) {
        let mut cmd = Command::new("cargo");

        cmd.args([
            "doc",
            "--no-deps",
            "--document-private-items",
            "--color",
            "always",
            "--no-default-features",
            "--features",
            &features.iter().join(","),
            "--package",
            &package.name,
        ])
        .env("RUSTDOCFLAGS", "--deny warnings");

        let status = cmd
            .spawn()
            .map_err(|x| error::CargoCommand::Spawn(x.into()))?
            .wait()
            .map_err(|x| error::CargoCommand::Wait(x.into()))?;

        if !status.success() {
            return Err(error::CargoCommand::Failure(
                package.name.into_inner(),
                features,
            ));
        }
    }

    Ok(())
}
