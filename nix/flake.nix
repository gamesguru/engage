{
  # Keep sorted, specify `ref`, and set `flake = false`.
  inputs = {
    crane = { url = "github:ipetkov/crane?ref=master"; flake = false; };
    fenix = { url = "github:nix-community/fenix?ref=main"; flake = false; };
    flake-compat = { url = "git+https://git.lix.systems/lix-project/flake-compat?ref=main"; flake = false; };
    nixpkgs = { url = "github:NixOS/nixpkgs?ref=nixos-unstable"; flake = false; };
    sprinkles = { url = "gitlab:charles/sprinkles?host=gitlab.computer.surgery&ref=v1"; flake = false; };
  };

  # Use the `default.nix` file next to this file instead.
  outputs = inputs: {};
}
