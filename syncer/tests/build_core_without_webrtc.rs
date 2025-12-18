use std::path::PathBuf;
use std::process::Command;

/// まだ feature 分離が無いため現時点では失敗する。
#[test]
fn core_build_without_webrtc_dependency_graph() {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("syncer has a parent directory")
        .to_path_buf();

    // 1) no-default-features で syncer の lib がビルドできること
    let status = Command::new("cargo")
        .arg("check")
        .args(["-p", "syncer", "--no-default-features", "--lib"])
        .current_dir(&workspace_root)
        .status()
        .expect("failed to run cargo check");
    assert!(
        status.success(),
        "cargo check -p syncer --no-default-features should succeed"
    );

    // 2) その依存グラフに webrtc クレートが含まれないこと
    let metadata = Command::new("cargo")
        .arg("metadata")
        .args(["--format-version", "1", "--no-default-features"])
        .current_dir(&workspace_root)
        .output()
        .expect("failed to run cargo metadata");
    assert!(
        metadata.status.success(),
        "cargo metadata must succeed for inspection"
    );

    let json: serde_json::Value =
        serde_json::from_slice(&metadata.stdout).expect("metadata should be valid json");

    let syncer_id = json["packages"]
        .as_array()
        .and_then(|packages| {
            packages
                .iter()
                .find(|pkg| pkg.get("name").and_then(|n| n.as_str()) == Some("syncer"))
        })
        .and_then(|pkg| pkg.get("id"))
        .and_then(|id| id.as_str())
        .expect("syncer package id must exist");

    let nodes = json["resolve"]
        .get("nodes")
        .and_then(|n| n.as_array())
        .expect("metadata resolve nodes must exist");

    let syncer_node = nodes
        .iter()
        .find(|node| node.get("id").and_then(|id| id.as_str()) == Some(syncer_id))
        .expect("syncer resolve node must exist");

    let has_webrtc_dep = syncer_node
        .get("deps")
        .and_then(|deps| deps.as_array())
        .map(|deps| {
            deps.iter().any(|dep| {
                dep.get("pkg")
                    .and_then(|pkg| pkg.as_str())
                    .map(|pkg| pkg.contains("webrtc"))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false);

    assert!(
        !has_webrtc_dep,
        "core build should exclude webrtc dependency when default features are disabled"
    );
}
