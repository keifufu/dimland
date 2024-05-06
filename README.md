# dimland

Dimland is a simple screen dimmer for Wayland. It overlays a transparent black layer on all display outputs, enabling additional brightness reduction, even when monitor backlights are set to 0%. It also includes a feature for drawing opaque screen corners, mimicking a rounded display.

## Installation

### NixOS

Import the flake and add `inputs.dimland.packages.${system}.default` to your packages

### Nix

Installing: `nix profile install github:keifufu/dimland`  
Running directly: `nix run github:keifufu/dimland -- --help`

### Other Distros

Build from source with `cargo build`

## Usage

`dimland --help`
