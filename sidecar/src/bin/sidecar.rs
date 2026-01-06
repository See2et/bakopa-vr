#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    // Minimal bootstrap; real server wiring will follow in later slices.
    let _app = sidecar::app::App::new().await?;
    Ok(())
}
