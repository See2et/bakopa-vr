#!/usr/bin/env bash
set -euo pipefail

PROJECT_PATH="${1:-client/godot}"
ARTIFACT_DIR="${2:-.takt-artifacts}"

if [ ! -f "${PROJECT_PATH}/project.godot" ]; then
  echo "skip: Godot project not found at ${PROJECT_PATH}" >&2
  exit 0
fi

case "$(uname -s)" in
  Linux)
    bash scripts/build-client-core-linux.sh
    if [ ! -f "${PROJECT_PATH}/bin/linux/libclient_core.so" ]; then
      echo "error: expected Linux client core not found at ${PROJECT_PATH}/bin/linux/libclient_core.so" >&2
      exit 1
    fi
    bash scripts/check_godot_headless_startup.sh "${PROJECT_PATH}" "${ARTIFACT_DIR}"
    ;;
  *)
    echo "skip: godot client startup check currently supports Linux only" >&2
    ;;
esac
