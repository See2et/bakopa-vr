use assert_cmd::cargo::cargo_bin_cmd;

#[test]
fn sidecar_binary_requires_port() {
    let mut cmd = cargo_bin_cmd!("sidecar");
    cmd.env("SIDECAR_TOKEN", "TEST_TOKEN_BINARY");
    cmd.env_remove("SIDECAR_PORT");
    cmd.assert().failure();
}
