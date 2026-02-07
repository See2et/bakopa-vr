#!/usr/bin/env bash
set -euo pipefail

arch="${CLIENT_CORE_MACOS_ARCH:-}"
profile="debug"

for ((i = 1; i <= $#; i++)); do
  arg="${!i}"
  if [[ "$arg" == "--release" ]]; then
    profile="release"
  elif [[ "$arg" == "--profile" ]]; then
    next_index=$((i + 1))
    if [[ $next_index -le $# ]]; then
      profile="${!next_index}"
    fi
  elif [[ "$arg" == "--arch" ]]; then
    next_index=$((i + 1))
    if [[ $next_index -le $# ]]; then
      arch="${!next_index}"
    fi
  fi
done

if [[ -z "$arch" ]]; then
  case "$(uname -m)" in
    x86_64) arch="x86_64" ;;
    arm64|aarch64) arch="arm64" ;;
    *)
      echo "error: unsupported host arch '$(uname -m)'; pass --arch x86_64|arm64" >&2
      exit 1
      ;;
  esac
fi

case "$arch" in
  x86_64) target="x86_64-apple-darwin" ;;
  arm64) target="aarch64-apple-darwin" ;;
  *)
    echo "error: unsupported --arch '$arch' (expected x86_64 or arm64)" >&2
    exit 1
    ;;
esac

filtered_args=()
skip_next=0
for arg in "$@"; do
  if [[ "$skip_next" == "1" ]]; then
    skip_next=0
    continue
  fi
  if [[ "$arg" == "--arch" ]]; then
    skip_next=1
    continue
  fi
  filtered_args+=("$arg")
done

cargo build -p client-godot-adapter --target "$target" "${filtered_args[@]}"

src="target/${target}/${profile}/libclient_core.dylib"
dest_dir="client/godot/bin/macos/${arch}"
dest="${dest_dir}/libclient_core.dylib"

if [[ ! -f "$src" ]]; then
  echo "error: built library not found at $src" >&2
  echo "hint: verify toolchain/target availability for $target" >&2
  exit 1
fi

mkdir -p "$dest_dir"
cp -f "$src" "$dest"

if [[ -f "$dest" ]]; then
  echo "ok: copied macOS library (${arch}) -> $dest"
else
  echo "error: expected copied library not found at $dest" >&2
  exit 1
fi
