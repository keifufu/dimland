{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = with pkgs; [
    libxkbcommon
  ];
  PKG_CONFIG_PATH = "${pkgs.libxkbcommon.dev}/lib/pkgconfig";
}