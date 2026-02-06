{ pkgs ? import (builtins.fetchTarball "https://github.com/NixOS/nixpkgs/archive/nixos-24.11.tar.gz") {
    overlays = [
      (import (builtins.fetchTarball "https://github.com/oxalica/rust-overlay/archive/master.tar.gz"))
    ];
  }
}:
let
  rustToolchain = pkgs.rust-bin.stable.latest.default.override {
    targets = [ "x86_64-pc-windows-gnu" ];
  };
  pthreads = pkgs.pkgsCross.mingwW64.windows.pthreads;
in
pkgs.mkShell {
  packages = [
    rustToolchain
    pkgs.pkgsCross.mingwW64.stdenv.cc
    pthreads
  ];
  CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS =
    "-L native=${pthreads}/lib";
}
