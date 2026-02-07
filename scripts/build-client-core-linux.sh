#!/usr/bin/env bash
set -euo pipefail

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
  fi
done

cargo build -p client-godot-adapter "$@"

src="target/${profile}/libclient_core.so"
dest_dir="client/godot/bin/linux"
dest="${dest_dir}/libclient_core.so"

if [[ ! -f "$src" ]]; then
  echo "error: built library not found at $src" >&2
  echo "hint: run on Linux host or pass proper build args" >&2
  exit 1
fi

mkdir -p "$dest_dir"
cp -f "$src" "$dest"

if [[ -f "$dest" ]]; then
  echo "ok: copied Linux library -> $dest"
else
  echo "error: expected copied library not found at $dest" >&2
  exit 1
fi
