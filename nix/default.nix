{
  inputs =
    (import (
      let
        lock = builtins.fromJSON (builtins.readFile ./flake.lock);
        inherit (lock.nodes.flake-compat.locked) narHash rev url;
      in
      builtins.fetchTarball {
        url = "${url}/archive/${rev}.tar.gz";
        sha256 = narHash;
      }
    ) { src = ./.; }).inputs;

  __functor =
    self:

    # Keep sorted.
    {
      crane ? import self.inputs.crane {
        pkgs = nixpkgs;
      },
      fenix ? import self.inputs.fenix {
        pkgs = nixpkgs;
      },
      nix-filter ? import self.inputs.nix-filter,
      nixpkgs ? import self.inputs.nixpkgs {
        config.allowAliases = false;
      },
    }:

    nixpkgs.lib.makeScope nixpkgs.newScope (scope:
      # Keep sorted.
      {
        craneLib = crane.overrideToolchain (_: scope.toolchain);

        inherit nix-filter;

        pkgs = nixpkgs.lib.filesystem.packagesFromDirectoryRecursive {
          inherit (scope) callPackage;
          directory = ./pkgs;
        };

        shells = nixpkgs.lib.filesystem.packagesFromDirectoryRecursive {
          inherit (scope) callPackage;
          directory = ./shells;
        };

        stdenv = if nixpkgs.stdenv.isLinux
          then nixpkgs.stdenvAdapters.useMoldLinker nixpkgs.stdenv
          else nixpkgs.stdenv;

        # Keep sorted.
        toolchain = fenix.combine [
          fenix.latest.rustfmt
          fenix.stable.cargo
          fenix.stable.clippy
          fenix.stable.llvm-tools
          fenix.stable.rust-src
          fenix.stable.rustc
        ];
      }
    );
}
