# Contributing

## Development requirements

1. Install [Lix][lix] and enable the `nix-command` and `flakes` experimental
   features.
2. Install [direnv][direnv] and [nix-direnv][nix-direnv].
3. If using a graphical editor, install an extension to give it direnv support.
   If no such extension is available, `cd`ing into the project directory
   and launching the editor from the terminal should cause it to inherit the
   environment; though the editor will likely need to be restarted to propagate
   any changes to the direnv setup to the editor if any such changes are made.

[lix]: https://lix.systems/install/
[direnv]: https://direnv.net/docs/installation.html
[nix-direnv]: https://github.com/nix-community/nix-direnv#installation
