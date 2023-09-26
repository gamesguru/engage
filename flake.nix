{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs?ref=nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { self
    , nixpkgs
    , flake-utils

    , fenix
    , crane
    }: flake-utils.lib.eachDefaultSystem (system:
    let
      pkgs = nixpkgs.legacyPackages.${system};
      stdenv =
        if pkgs.stdenv.isLinux then
          pkgs.stdenvAdapters.useMoldLinker pkgs.stdenv
        else
          pkgs.stdenv;

      cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);

      mkToolchain = fenix.packages.${system}.combine;

      toolchain = fenix.packages.${system}.stable;

      buildToolchain = mkToolchain (with toolchain; [
        cargo
        rustc
      ]);

      devToolchain = mkToolchain (with toolchain; [
        cargo
        clippy
        rust-src
        rustc

        # Always use nightly rustfmt because most of its options are unstable
        fenix.packages.${system}.latest.rustfmt
      ]);

      builder =
        ((crane.mkLib pkgs).overrideToolchain buildToolchain).buildPackage;
    in
    {
      packages.default = builder {
        src = ./.;

        nativeBuildInputs = (with pkgs; [ installShellFiles ]);

        postInstall =
          let
            cmd = cargoToml.package.name;
          in
          "installShellCompletion --cmd ${cmd} " + builtins.concatStringsSep
            " "
            (builtins.map
              (shell: "--${shell} <($out/bin/${cmd} completions ${shell})")
              [ "bash" "zsh" "fish" ]
            );

        inherit stdenv;
      };

      devShells.default = (pkgs.mkShell.override { inherit stdenv; }) {
        # Rust Analyzer needs to be able to find the path to default crate
        # sources, and it can read this environment variable to do so. The
        # `rust-src` component is required in order for this to work.
        RUST_SRC_PATH = "${devToolchain}/lib/rustlib/src/rust/library";

        # Development tools
        nativeBuildInputs = [
          devToolchain
        ] ++ (with pkgs; [
          cargo-insta
          graphviz
          mdbook
          nixpkgs-fmt
        ]) ++ (with pkgs.nodePackages; [
          markdownlint-cli
        ]);

        # Give GraphViz access to the same fonts locally and in CI
        FONTCONFIG_FILE = pkgs.makeFontsConf {
          fontDirectories = with pkgs; [
            dejavu_fonts
          ];
        };
      };

      checks = {
        packagesDefault = self.packages.${system}.default;
        devShellsDefault = self.devShells.${system}.default;
      };
    });
}
