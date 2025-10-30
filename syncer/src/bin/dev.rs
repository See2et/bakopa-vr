#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    dbg!(&args);
    todo!("write start_syncer")
}
