{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { self
    , nixpkgs
    , flake-utils

    , fenix
    , naersk
    }: flake-utils.lib.eachDefaultSystem (system:
    let
      pkgs = nixpkgs.legacyPackages.${system};
      toolchain = fenix.packages.${system}.stable;
    in
    {
      packages.default = (pkgs.callPackage naersk {
        inherit (toolchain) rustc cargo;
      }).buildPackage {
        src = ./.;
      };

      devShells.default = pkgs.mkShell {
        # Rust Analyzer needs to be able to find the path to default crate
        # sources, and it can read this environment variable to do so
        RUST_SRC_PATH = "${toolchain.rust-src}/lib/rustlib/src/rust/library";

        # Development tools
        nativeBuildInputs = (with toolchain; [
          cargo
          clippy
          rust-src
          rustc

          # Always use nightly rustfmt because most of its options are unstable
          fenix.packages.${system}.latest.rustfmt
        ]) ++ (with pkgs; [
          file
          ncurses
          nixpkgs-fmt
          shellcheck
          shfmt
        ]) ++ (with pkgs.nodePackages; [
          markdownlint-cli
        ]);
      };

      checks = {
        packagesDefault = self.packages.${system}.default;
        devShellsDefault = self.devShells.${system}.default;
      };
    });
}
