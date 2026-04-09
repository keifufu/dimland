# dim (bash wrapper)

dim - a wrapper for dimland with persistent per-output state saving/loading

```
Usage: dim [OPTIONS] [COMMAND]

Commands:
  more           Increase alpha by the current STEP size
  less           Decrease alpha by the current STEP size
  toggle         Toggle (jump) between current and alt-alpha
  toggle 0.25    Toggle (jump) to a specific alt-alpha value
  status         Show current settings
  stop           Stop dimland

Options:
  -a, --alpha <ALPHA>      Transparency level (0.0 transparent, 1.0 opaque)
      --allow-opaque       Allow alpha to go beyond THRESHOLD (90% by default)
  -r, --radius <RADIUS>    Corner radius (default 6)
  -s, --step <STEP>        Set dimming STEP size (persisted)
  -o, --output <OUTPUT>    Output to control (ex. DP-1)
  -h, --help               Print help
  -V, --version            Print version
```

## Usage examples:

```
dim                      → apply last saved state (global or per-output)
dim -a 0.2               → set alpha of a transparent black dimming layer
dim -a 0.4 -o DP-1       → apply alpha to a specific output (e.g. DP-1, HDMI-A-1)
dim -r 6                 → set radius of rounded screen corners
dim -s 0.08              → set dimming STEP size (persisted, per-output)
dim more                 → increase alpha by current STEP size
dim less                 → decrease alpha by current STEP size
dim toggle               → toggle (jump) between current and alt-alpha
dim toggle 0.25          → toggle (jump) to a specific alt-alpha value
dim status               → show current settings
dim stop                 → stop dimland
```

## Usage notes:
- Automatic `--allow-opaque` is applied after reaching `DIMMING TRESHOLD` 5 times in a row
- `CLI args` take precedence over `STATE file` (`CLI` > `STATE` > `DEFAULTS`)
- User may configure the `DEFAULTS` to his linking in the `DEFAULTS (D_ prefix)` section
- `ALPHA PROCESSING` and `DIMMING TRESHOLD` can be adjusted in the `ALPHA PROCESSING` section



# dimland (screen dimmer for Wayland)

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
  -o, --output <OUTPUT>  Output to control (ex. DP-1)
  -h, --help             Print help
  -V, --version          Print version
```

[Nix package manager]: https://nixos.org/download/
[Rust]: https://ww.rust-lang.org/
