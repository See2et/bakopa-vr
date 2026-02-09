{ pkgs, pkgsUnstable ? null }:
let
  rustVersion = "1.90.0";
  rustToolchain = pkgs.rust-bin.stable.${rustVersion}.default.override {
    targets = [ "x86_64-pc-windows-gnu" ];
  };
  pthreads = pkgs.pkgsCross.mingwW64.windows.pthreads;
  godotPackage =
    if pkgsUnstable != null && pkgsUnstable.stdenv.isLinux && pkgsUnstable ? godot_4_5 then
      pkgsUnstable.godot_4_5
    else
      pkgs.godot_4;
  shellHook = ''
    export LIBCLANG_PATH=${pkgs.libclang.lib}/lib
  '';
in
{
  default = pkgs.mkShell {
    packages = [
      rustToolchain
      pkgs.libclang
    ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
      godotPackage
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
