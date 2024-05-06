# dimland

stupidly simple wayland screen dimmer  
this is just a transparent black overlay, it does not touch the backlight  
i made this because my monitors are still too bright for me at 0% at night

this can also draw opaque screen corners, to imitate a rounded display

## installation

Build it yourself or use the nix flake.
To build run `cargo build` and copy the resulting `target/debug/dimland` into your PATH.

Installing it with regular nix:

    nix profile install github:keifufu/dimland

You can also run it without installing:

    nix run github:keifufu/dimland -- --help

## usage

    dimland --help
