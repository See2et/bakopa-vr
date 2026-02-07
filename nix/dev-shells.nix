{ pkgs }:
let
  rustVersion = "1.90.0";
  rustToolchain = pkgs.rust-bin.stable.${rustVersion}.default.override {
    targets = [ "x86_64-pc-windows-gnu" ];
  };
  pthreads = pkgs.pkgsCross.mingwW64.windows.pthreads;
  shellHook = ''
    export LIBCLANG_PATH=${pkgs.libclang.lib}/lib
  '';
in
{
  default = pkgs.mkShell {
    packages = [
      rustToolchain
      pkgs.libclang
    ];
    inherit shellHook;
  };

  windows = pkgs.mkShell {
    packages = [
      rustToolchain
      pkgs.pkgsCross.mingwW64.stdenv.cc
      pthreads
      pkgs.libclang
    ];
    CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS =
      "-L native=${pthreads}/lib";
    inherit shellHook;
  };
}
