# dimland

Dimland is a simple screen dimmer for Wayland. It overlays a transparent black layer on all display outputs, enabling additional brightness reduction, even when the monitor backlight is to 0%. It also includes a feature for drawing opaque screen corners, mimicking a rounded display.

## Installation

### Nix

The preferred way to install dimland is using the [Nix package manager].

```bash
nix profile install github:keifufu/dimland
```

<details>
<summary>NixOS</summary>

This assumes you use home manager with flakes.

- Add `github:keifufu/dimland` to your inputs
- Import `inputs.dimland.homeManagerModules.dimland`

```nix
  programs.dimland = {
    enable = true;
    # If you want to start dimland on startup
    service = {
      enable = true;
      alpha = 0;
      radius = 20;
      # Specify target to start after
      after = "hyprland-started.path";
    };
  };

  # Assuming you use Hyprland, start after its socket exists
  systemd.user.paths.hyprland-started = {
    Unit.Description = "Watch for Hyprland to start";
    Path.PathExists = "%t/hypr";
    Install.WantedBy = [ "default.target" ];
  };
```

</details>

### Building Manually

> [!IMPORTANT]
>
> - Ensure you have [Rust] installed.
> - The system libraries `libxkbcommon` and `libwayland` are required.

```bash
cargo build --release
```

The resulting binary will be in `./target/release/dimland`

## Usage

```
Usage: dimland [OPTIONS] [COMMAND]

Commands:
  stop  Stops the program
  help  Print this message or the help of the given subcommand(s)

Options:
  -a, --alpha <ALPHA>    Transparency level (0.0 transparent, 1.0 opaque, default 0.5, max 0.9)
      --allow-opaque     Allow alpha to go beyond 0.9
  -r, --radius <RADIUS>  Corner radius (default 0)
  -h, --help             Print help
  -V, --version          Print version
```

[Nix package manager]: https://nixos.org/download/
[Rust]: https://ww.rust-lang.org/
