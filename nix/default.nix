{ sprinkles ? null }:

let
  source = import ./lon.nix;

  # Keep sorted.
  input = source: {
    crane = import source.crane {
      pkgs = (input source).nixpkgs;
    };
    fenix = import source.fenix {
      pkgs = (input source).nixpkgs;
    };
    nixpkgs = import source.nixpkgs {
      config.allowAliases = false;
    };
    sprinkles = if sprinkles == null
      then import source.sprinkles
      else sprinkles;
  };
in

(input source).sprinkles.new {
  inherit input source;

  output = self:
    let
      inherit (self.input) crane fenix nixpkgs;
      inherit (self.input.nixpkgs.lib.customisation) makeScope;

      # Keep sorted.
      toolchain = fenix.combine (with fenix; [
        latest.rustfmt
        stable.cargo
        stable.clippy
        stable.llvm-tools
        stable.rust-src
        stable.rustc
      ]);
    in
    {
      package = makeScope nixpkgs.newScope (scope:
        let
          craneLib = crane.overrideToolchain (_: toolchain);
        in
        {
          default = scope.callPackage ./package/default {
            inherit craneLib;
          };
        }
      );

      shell = makeScope self.output.package.newScope (scope: {
        default = scope.callPackage ./shell/default {
          inherit (self.output.package) default;
          inherit toolchain;
        };
      });
    };
}
