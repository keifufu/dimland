{
  description = "dimland";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
  };
  outputs = inputs@{ self, flake-parts, ... }: 
  flake-parts.lib.mkFlake { inherit inputs; } {
    systems = [
      "x86_64-linux"
      "aarch64-linux"
    ];
    perSystem = { pkgs, system, ...}: let
      manifest = (pkgs.lib.importTOML ./Cargo.toml).package;
    in {
      packages.default = pkgs.rustPlatform.buildRustPackage rec {
        pname = manifest.name;
        version = manifest.version;
        cargoLock.lockFile = ./Cargo.lock;
        src = pkgs.lib.cleanSource ./.;
        buildInputs = with pkgs; [ libxkbcommon ];
        nativeBuildInputs = with pkgs; [ pkg-config ];
      };
      devShells.default = pkgs.mkShell {
        shellHook = "exec $SHELL";
        buildInputs = with pkgs; [ libxkbcommon ];
        PKG_CONFIG_PATH = "${pkgs.libxkbcommon.dev}/lib/pkgconfig";
      };
    };
    flake = {
      homeManagerModules.dimland = { config, pkgs, lib, ... }: let
        inherit (pkgs.stdenv.hostPlatform) system;
      in {
        options.programs.dimland = {
          enable = lib.mkOption {
            type = lib.types.bool;
            default = false;
            description = "Install dimland package";
          };
          package = lib.mkOption {
            type = lib.types.package;
            default = self.packages.${system}.default;
            description = "Dimland package to install";
          };
          service.enable = lib.mkOption {
            type = lib.types.bool;
            default = false;
            description = "Enable dimland service";
          };
          service.alpha = lib.mkOption {
            type = lib.types.number;
            default = 0.5;
            description = "Dimland service initial alpha value";
          };
          service.radius = lib.mkOption {
            type = lib.types.number;
            default = 0.0;
            description = "Dimland service initial radius value";
          };
        };
        config = lib.mkIf config.programs.dimland.enable {
          home.packages = [ config.programs.dimland.package ];
          systemd.user.services.dimland = lib.mkIf config.programs.dimland.service.enable {
            Unit = {
              Description = "dimland service";
              PartOf = [ "graphical-session.target" ];
              After = [ "graphical-session-pre.target" ];
            };
            Service = {
              ExecStart = "${config.programs.dimland.package}/bin/dimland --alpha ${toString config.programs.dimland.service.alpha} --radius ${toString config.programs.dimland.service.radius} --detached";
              Restart = "on-failure";
              RestartSec = config.programs.dimland.service.restartSec;
            };
            Install.WantedBy = [ "graphical-session.target" ];
          };
        };
      };
    };
  };
}
