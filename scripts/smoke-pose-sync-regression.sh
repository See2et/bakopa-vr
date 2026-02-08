#!/usr/bin/env bash
set -euo pipefail

iterations="${ITERATIONS:-3}"

echo "[smoke] iterations=${iterations}"
for i in $(seq 1 "${iterations}"); do
  echo "[smoke] round=${i} stage=input-desktop"
  cargo test -p client-godot-adapter --all-targets desktop_input_normalization_maps_wasd_and_mouse_to_common_semantics

  echo "[smoke] round=${i} stage=input-vr"
  cargo test -p client-godot-adapter --all-targets vr_input_normalization_maps_controller_input_to_common_semantics

  echo "[smoke] round=${i} stage=receive"
  cargo test -p client-domain --all-targets two_participant_sync_flow_handles_join_rejoin_and_stale_pose

  echo "[smoke] round=${i} stage=projection"
  cargo test -p client-godot-adapter --all-targets remote_pose_projection_follows_frame_updates_and_removals
done

echo "[smoke] smoke regression complete"
