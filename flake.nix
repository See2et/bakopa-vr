{
  description = "bakopa-vr dev shell";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.11";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, rust-overlay }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ rust-overlay.overlays.default ];
      };
      rustToolchain = pkgs.rust-bin.stable.latest.default.override {
        targets = [ "x86_64-pc-windows-gnu" ];
      };
      pthreads = pkgs.pkgsCross.mingwW64.windows.pthreads;
    in
    {
      devShells.${system} = {
        default = pkgs.mkShell {
          packages = [
            rustToolchain
          ];
        };
        windows = pkgs.mkShell {
          packages = [
            rustToolchain
            pkgs.pkgsCross.mingwW64.stdenv.cc
            pthreads
          ];
          CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS =
            "-L native=${pthreads}/lib";
        };
      };
    };
}
