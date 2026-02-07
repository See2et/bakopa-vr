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

profile="debug"
for ((i = 1; i <= $#; i++)); do
  arg="${!i}"
  if [[ "$arg" == "--release" ]]; then
    profile="release"
  elif [[ "$arg" == --profile=* ]]; then
    profile="${arg#*=}"
  elif [[ "$arg" == "--profile" ]]; then
    next_index=$((i + 1))
    if [[ $next_index -le $# ]]; then
      profile="${!next_index}"
    fi
  fi
done

case "$profile" in
  dev|debug)
    profile="debug"
    ;;
  release)
    profile="release"
    ;;
  *)
    ;;
esac

filtered_args=()
skip_next=0
for arg in "$@"; do
  if [[ "$skip_next" == "1" ]]; then
    skip_next=0
    continue
  fi
  if [[ "$arg" == "--release" ]]; then
    continue
  fi
  if [[ "$arg" == "--profile" ]]; then
    skip_next=1
    continue
  fi
  if [[ "$arg" == --profile=* ]]; then
    continue
  fi
  filtered_args+=("$arg")
done

profile_args=()
case "$profile" in
  release)
    profile_args=(--release)
    ;;
  debug)
    profile_args=()
    ;;
  *)
    profile_args=(--profile "$profile")
    ;;
esac

env \
  CC=x86_64-w64-mingw32-gcc \
  CXX=x86_64-w64-mingw32-g++ \
  AR=x86_64-w64-mingw32-ar \
  RANLIB=x86_64-w64-mingw32-ranlib \
  cargo build -p client-godot-adapter --target x86_64-pc-windows-gnu \
  ${profile_args[@]+"${profile_args[@]}"} \
  ${filtered_args[@]+"${filtered_args[@]}"}

src="target/x86_64-pc-windows-gnu/${profile}/client_core.dll"
dest_dir="client/godot/bin/windows"
dest="${dest_dir}/client_core.dll"

if [[ ! -f "$src" ]]; then
  echo "error: built DLL not found at $src" >&2
  echo "hint: verify build profile/target and cargo output" >&2
  exit 1
fi

mkdir -p "$dest_dir"
cp -f "$src" "$dest"

if [[ -f "$dest" ]]; then
  echo "ok: copied DLL -> $dest"
else
  echo "error: expected copied DLL not found at $dest" >&2
  exit 1
fi
