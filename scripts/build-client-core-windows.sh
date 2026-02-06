#!/usr/bin/env bash
set -euo pipefail

if ! command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
  echo "error: x86_64-w64-mingw32-gcc not found in PATH" >&2
  echo "hint: enter the dev shell that provides mingw before running this script" >&2
  exit 1
fi

if [[ -z "${CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS:-}" ]]; then
  echo "warning: CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS is not set" >&2
  echo "warning: if you rely on pthreads-w32, set -L native=.../lib before building" >&2
fi

env \
  CC=x86_64-w64-mingw32-gcc \
  CXX=x86_64-w64-mingw32-g++ \
  AR=x86_64-w64-mingw32-ar \
  RANLIB=x86_64-w64-mingw32-ranlib \
  cargo build -p client-godot-adapter --target x86_64-pc-windows-gnu "$@"

# DLL placement is handled in client/godot-adapter/build.rs.
dest="client/godot/bin/windows/client_core.dll"
if [[ -f "$dest" ]]; then
  echo "ok: copied DLL -> $dest"
else
  echo "error: expected DLL not found at $dest" >&2
  echo "hint: check client/godot-adapter/build.rs copy logic and build output" >&2
  exit 1
fi
