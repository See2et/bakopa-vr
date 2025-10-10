use std::process::Command;

#[test]
#[ignore = "Runs the compiled peer-cli binary"]
fn peer_cli_help_runs() {
    let exe = std::env::var("CARGO_BIN_EXE_peer-cli").expect("binary path");
    let output = Command::new(exe).arg("--help").output().expect(
        "execute peer-cli --help",
    );
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Peer-to-peer ping/pong tester"));
}
