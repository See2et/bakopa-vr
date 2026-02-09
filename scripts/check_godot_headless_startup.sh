#!/usr/bin/env bash
set -euo pipefail

PROJECT_PATH="${1:-client/godot}"
ARTIFACT_DIR="${2:-.takt-artifacts}"
IMPORT_LOG="${ARTIFACT_DIR}/godot-import.log"
STARTUP_LOG="${ARTIFACT_DIR}/godot-startup.log"
IMPORT_CONSOLE_LOG="${ARTIFACT_DIR}/godot-import.console.log"
STARTUP_CONSOLE_LOG="${ARTIFACT_DIR}/godot-startup.console.log"
RUNTIME_PROBE_CONSOLE_LOG="${ARTIFACT_DIR}/godot-runtime-probe.console.log"
COMBINED_LOG="${ARTIFACT_DIR}/godot-headless.log"
STAGE_LOG="${ARTIFACT_DIR}/godot-startup-stage.log"
STARTUP_MODE="${GODOT_STARTUP_MODE:-desktop}"
PROBE_QUIT_AFTER="${GODOT_PROBE_QUIT_AFTER:-20}"
LIB_PATH="${PROJECT_PATH}/bin/linux/libclient_core.so"
USER_LOG_COPY="${ARTIFACT_DIR}/godot-user.log"

if [ ! -f "${PROJECT_PATH}/project.godot" ]; then
  echo "skip: Godot project not found at ${PROJECT_PATH}" >&2
  exit 0
fi

APP_NAME="$(sed -n 's/^config\/name="\([^"]*\)".*/\1/p' "${PROJECT_PATH}/project.godot" | head -n 1)"
USER_LOG_PATH=""
if [ -n "${APP_NAME}" ]; then
  USER_LOG_PATH="${HOME}/.local/share/godot/app_userdata/${APP_NAME}/logs/godot.log"
fi

mkdir -p "${ARTIFACT_DIR}"
rm -f "${STAGE_LOG}" "${IMPORT_CONSOLE_LOG}" "${STARTUP_CONSOLE_LOG}" "${RUNTIME_PROBE_CONSOLE_LOG}" "${USER_LOG_COPY}"

log_stage() {
  local stage="$1"
  local status="$2"
  local detail="$3"
  printf "stage=%s status=%s mode=%s library_path=%s detail=%s\n" \
    "${stage}" "${status}" "${STARTUP_MODE}" "${LIB_PATH}" "${detail}" | tee -a "${STAGE_LOG}" >/dev/null
}

resolve_godot_bin() {
  if [ -n "${GODOT_BIN:-}" ] && [ -x "${GODOT_BIN}" ]; then
    printf "%s\n" "${GODOT_BIN}"
    return 0
  fi

  if command -v godot >/dev/null 2>&1; then
    command -v godot
    return 0
  fi
  if command -v godot4 >/dev/null 2>&1; then
    command -v godot4
    return 0
  fi

  for candidate in "${PROJECT_PATH}/bin/linux/godot" "${PROJECT_PATH}/bin/linux/godot4" "${PROJECT_PATH}"/bin/linux/Godot_v4*; do
    if [ -x "${candidate}" ]; then
      printf "%s\n" "${candidate}"
      return 0
    fi
  done

  return 1
}

if ! GODOT_BIN="$(resolve_godot_bin)"; then
  log_stage "resolve_godot_bin" "skip" "godot_binary_not_found"
  echo "skip: Godot binary not found (expected GODOT_BIN, 'godot', 'godot4', or local bin)" >&2
  exit 0
fi

log_stage "resolve_godot_bin" "ok" "${GODOT_BIN}"

run_godot_stage() {
  local stage="$1"
  local logfile="$2"
  local console_log="$3"
  shift 3
  set +e
  "${GODOT_BIN}" "$@" --log-file "${logfile}" >"${console_log}" 2>&1
  local code=$?
  set -e
  if [ ${code} -eq 0 ]; then
    log_stage "${stage}" "ok" "exit_code=0"
    return 0
  fi

  if [ ${code} -eq 139 ]; then
    log_stage "${stage}" "crash" "exit_code=139_sigsegv"
    echo "error: Godot crashed with SIGSEGV at stage=${stage}" >&2
  else
    log_stage "${stage}" "error" "exit_code=${code}"
    echo "error: Godot failed at stage=${stage} exit_code=${code}" >&2
  fi
  return 1
}

run_godot_probe_stage() {
  local stage="$1"
  local console_log="$2"
  shift 2
  set +e
  RUST_BACKTRACE=full RUST_LOG="${RUST_LOG:-trace}" "${GODOT_BIN}" "$@" >"${console_log}" 2>&1
  local code=$?
  set -e
  if [ ${code} -eq 0 ]; then
    log_stage "${stage}" "ok" "exit_code=0"
    return 0
  fi
  log_stage "${stage}" "error" "exit_code=${code}"
  echo "error: Godot runtime probe failed at stage=${stage} exit_code=${code}" >&2
  return 1
}

import_exit=0
startup_exit=0
probe_exit=0

if run_godot_stage "import" "${IMPORT_LOG}" \
  "${IMPORT_CONSOLE_LOG}" \
  --headless \
  --path "${PROJECT_PATH}" \
  --import \
  --quit \
  "--${STARTUP_MODE}"; then
  :
else
  import_exit=$?
fi

if run_godot_stage "startup" "${STARTUP_LOG}" \
  "${STARTUP_CONSOLE_LOG}" \
  --headless \
  --path "${PROJECT_PATH}" \
  --quit-after 5 \
  "--${STARTUP_MODE}"; then
  :
else
  startup_exit=$?
fi

if run_godot_probe_stage "runtime_probe" "${RUNTIME_PROBE_CONSOLE_LOG}" \
  --headless \
  --path "${PROJECT_PATH}" \
  -d \
  --quit-after "${PROBE_QUIT_AFTER}" \
  "--${STARTUP_MODE}"; then
  :
else
  probe_exit=$?
fi

if [ -n "${USER_LOG_PATH}" ] && [ -f "${USER_LOG_PATH}" ]; then
  cp -f "${USER_LOG_PATH}" "${USER_LOG_COPY}"
  log_stage "user_log" "ok" "${USER_LOG_PATH}"
else
  log_stage "user_log" "skip" "user_log_not_found"
fi

: > "${COMBINED_LOG}"
for source_log in \
  "${IMPORT_LOG}" \
  "${STARTUP_LOG}" \
  "${IMPORT_CONSOLE_LOG}" \
  "${STARTUP_CONSOLE_LOG}" \
  "${RUNTIME_PROBE_CONSOLE_LOG}" \
  "${USER_LOG_COPY}" \
  "${STAGE_LOG}"
do
  if [ -f "${source_log}" ]; then
    cat "${source_log}" >> "${COMBINED_LOG}"
    printf "\n" >> "${COMBINED_LOG}"
  fi
done

if [ "${STARTUP_MODE}" = "desktop" ]; then
  if grep -q "OpenXR was requested but failed to start" "${COMBINED_LOG}"; then
    sed -i '/OpenXR was requested but failed to start/d' "${COMBINED_LOG}"
    log_stage "openxr_warning_filter" "ok" "desktop_known_warning_removed"
  else
    log_stage "openxr_warning_filter" "skip" "pattern_not_found"
  fi
else
  log_stage "openxr_warning_filter" "skip" "vr_mode"
fi

if command -v rg >/dev/null 2>&1; then
  MATCH_CMD=(
    rg -n
    "SCRIPT ERROR:|\\bERROR:\\b|^E[[:space:]]+[0-9]|NG: GDExtension|load_extension failed|godot-rust function call failed|function panicked|render projection failed|OnEditor field .*initialized|Program crashed with signal"
  )
else
  MATCH_CMD=(
    grep -nE
    "SCRIPT ERROR:|\\bERROR:\\b|^E[[:space:]]+[0-9]|NG: GDExtension|load_extension failed|godot-rust function call failed|function panicked|render projection failed|OnEditor field .*initialized|Program crashed with signal"
  )
fi

if "${MATCH_CMD[@]}" "${COMBINED_LOG}" >/dev/null; then
  log_stage "log_scan" "error" "startup_error_pattern_detected"
  echo "Godot startup errors detected. See ${COMBINED_LOG}" >&2
  "${MATCH_CMD[@]}" "${COMBINED_LOG}" >&2 || true
  exit 1
fi

if [ "${import_exit}" -ne 0 ] || [ "${startup_exit}" -ne 0 ] || [ "${probe_exit}" -ne 0 ]; then
  log_stage "log_scan" "error" "godot_stage_failed_without_detectable_pattern"
  echo "Godot startup failed without matching known error patterns. See ${COMBINED_LOG}" >&2
  exit 1
fi

log_stage "log_scan" "ok" "no_error_pattern_detected"
echo "Godot headless startup check passed (${COMBINED_LOG})"
