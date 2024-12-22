{
  # Keep sorted, specify `ref`, and set `flake = false`.
  inputs = {
    crane = { url = "github:ipetkov/crane?ref=master"; flake = false; };
    fenix = { url = "github:nix-community/fenix?ref=main"; flake = false; };
    flake-compat = { url = "git+https://git.lix.systems/lix-project/flake-compat?ref=main"; flake = false; };
    nix-filter = { url = "github:numtide/nix-filter?ref=main"; flake = false; };
    nixpkgs = { url = "github:NixOS/nixpkgs?ref=nixos-unstable"; flake = false; };
  };

  # Use the `default.nix` file next to this file instead.
  outputs = inputs: {};
}
