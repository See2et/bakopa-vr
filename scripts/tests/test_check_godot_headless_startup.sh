#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT_PATH="${ROOT_DIR}/scripts/check_godot_headless_startup.sh"

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

PROJECT_PATH="${TMP_DIR}/godot_project"
ARTIFACT_DIR="${TMP_DIR}/artifacts"
mkdir -p "${PROJECT_PATH}" "${ARTIFACT_DIR}"
cat > "${PROJECT_PATH}/project.godot" <<'EOF'
config_version=5
EOF

assert_contains() {
  local file="$1"
  local pattern="$2"
  if ! grep -q -- "${pattern}" "${file}"; then
    echo "assertion failed: pattern '${pattern}' not found in ${file}" >&2
    exit 1
  fi
}

echo "[case1] missing godot binary is handled as diagnostic skip"
GODOT_BIN="" /bin/bash "${SCRIPT_PATH}" "${PROJECT_PATH}" "${ARTIFACT_DIR}" >"${TMP_DIR}/case1.out" 2>"${TMP_DIR}/case1.err"
assert_contains "${ARTIFACT_DIR}/godot-startup-stage.log" "stage=resolve_godot_bin status=skip"

echo "[case2] fake godot binary executes import/startup with desktop mode"
FAKE_GODOT="${TMP_DIR}/fake_godot.sh"
cat > "${FAKE_GODOT}" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
log_file=""
mode_seen="no"
args=()
while [ "$#" -gt 0 ]; do
  case "$1" in
    --log-file)
      shift
      log_file="$1"
      ;;
    --desktop)
      mode_seen="yes"
      ;;
    *)
      args+=("$1")
      ;;
  esac
  shift || true
done
if [ -n "${log_file}" ]; then
  printf "mode_desktop=%s\n" "${mode_seen}" > "${log_file}"
fi
exit 0
EOF
chmod +x "${FAKE_GODOT}"

GODOT_BIN="${FAKE_GODOT}" bash "${SCRIPT_PATH}" "${PROJECT_PATH}" "${ARTIFACT_DIR}" >"${TMP_DIR}/case2.out" 2>"${TMP_DIR}/case2.err"
assert_contains "${ARTIFACT_DIR}/godot-import.log" "mode_desktop=yes"
assert_contains "${ARTIFACT_DIR}/godot-startup.log" "mode_desktop=yes"
assert_contains "${ARTIFACT_DIR}/godot-startup-stage.log" "stage=log_scan status=ok"

echo "[case3] desktop mode suppresses known OpenXR startup warning noise"
cat > "${FAKE_GODOT}" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
log_file=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --log-file)
      shift
      log_file="$1"
      ;;
  esac
  shift || true
done
if [ -n "${log_file}" ]; then
  printf "OpenXR was requested but failed to start\n" > "${log_file}"
fi
exit 0
EOF
chmod +x "${FAKE_GODOT}"

GODOT_BIN="${FAKE_GODOT}" bash "${SCRIPT_PATH}" "${PROJECT_PATH}" "${ARTIFACT_DIR}" >"${TMP_DIR}/case3.out" 2>"${TMP_DIR}/case3.err"
assert_contains "${ARTIFACT_DIR}/godot-startup-stage.log" "stage=openxr_warning_filter status=ok"

echo "ok: check_godot_headless_startup"
