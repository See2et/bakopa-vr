use anyhow::Context;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let app = sidecar::app::App::new()
        .await
        .context("initialize sidecar app")?;
    if app.bind_addr().port() == 0 {
        anyhow::bail!("SIDECAR_PORT is required for sidecar binary");
    }
    let listener = tokio::net::TcpListener::bind(app.bind_addr())
        .await
        .context("bind sidecar listen addr")?;
    axum::serve(listener, app.router())
        .await
        .context("serve sidecar")?;
    Ok(())
}
