# Contributing

## Development requirements

1. Install [Lix], [direnv], and [nix-direnv].

2. Enable Lix's [`nix-command`] experimental feature.

3. If using a graphical editor, ensure it has direnv support, e.g. by installing
   an extension. If no support is available, changing directories into the
   project and launching the editor from the terminal should cause it to inherit
   the environment; though the editor will likely need to be restarted to
   propagate any changes to the direnv setup to the editor if any such changes
   are made.

[Lix]: https://lix.systems/
[`nix-command`]: https://docs.lix.systems/manual/lix/stable/contributing/experimental-features.html#xp-feature-nix-command
[direnv]: https://direnv.net
[nix-direnv]: https://github.com/nix-community/nix-direnv
