{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { self
    , nixpkgs
    , flake-utils

    , fenix
    }: flake-utils.lib.eachDefaultSystem (system:
    let
      pkgs = nixpkgs.legacyPackages.${system};
      rust = fenix.packages.${system};
      cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
    in
    {
      packages.default = (pkgs.makeRustPlatform {
        inherit (rust.stable) cargo rustc;
      }).buildRustPackage {
        pname = cargoToml.package.name;
        version = cargoToml.package.version;

        cargoLock.lockFile = ./Cargo.lock;

        src = builtins.filterSource
          # Exclude `target` because it's huge
          (path: type: !(type == "directory" && baseNameOf path == "target"))
          ./.;

        # This is disabled so CI can be impure and not break Nix builds
        doCheck = false;
      };

      devShells.default = self.packages.${system}.default.overrideAttrs (old: {
        # Rust Analyzer needs to be able to find the path to default crate
        # sources, and it can read this environment variable to do so
        RUST_SRC_PATH = "${rust.stable.rust-src}/lib/rustlib/src/rust/library";

        nativeBuildInputs = (old.nativeBuildInputs or [ ]) ++ (with pkgs; [
          file
          ncurses
          nixpkgs-fmt
          shellcheck
          shfmt
        ]) ++ (with rust; [
          latest.rustfmt
          stable.clippy
          stable.rust-src
        ]) ++ (with pkgs.nodePackages; [
          markdownlint-cli
        ]);
      });

      checks = {
        packagesDefault = self.packages.${system}.default;
        devShellsDefault = self.devShells.${system}.default;
      };
    });
}
