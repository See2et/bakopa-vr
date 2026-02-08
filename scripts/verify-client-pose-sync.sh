#!/usr/bin/env bash
set -euo pipefail

run_stage() {
  local stage="$1"
  shift
  echo "[verify] stage=${stage}"
  "$@"
}

run_stage "input" cargo test -p client-godot-adapter --all-targets desktop_input_normalization_maps_wasd_and_mouse_to_common_semantics
run_stage "input" cargo test -p client-godot-adapter --all-targets vr_input_normalization_maps_controller_input_to_common_semantics
run_stage "send" cargo test -p client-domain --all-targets pose_sync_coordinator_recovers_send_after_room_ready_transition
run_stage "receive" cargo test -p client-domain --all-targets two_participant_sync_flow_handles_join_rejoin_and_stale_pose
run_stage "projection" cargo test -p client-godot-adapter --all-targets remote_pose_projection_follows_frame_updates_and_removals

echo "[verify] verification complete"
