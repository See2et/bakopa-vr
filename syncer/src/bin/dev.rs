use clap::Parser;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use syncer::{NodeConfig, SyncerNode};

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long, default_value = "")]
    listen: String,
    #[arg(short, long, default_value = "")]
    private_key_path: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let addr = if args.listen.is_empty() {
        let default = "127.0.0.1:8080".to_string();
        eprintln!(
            "--listen が空なので、デフォルト値 {} を使用します。",
            default
        );
        default
    } else {
        args.listen
    };

    println!("listen = {}", addr);

    let socket_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
    if !args.private_key_path.is_empty() {
        let path = PathBuf::from(&args.private_key_path);
        let node_config = NodeConfig::new(socket_addr).with_private_key_path(path);
        let _ = SyncerNode::start(&node_config).await;
    } else {
        let node_config = NodeConfig::new(socket_addr);
        let _ = SyncerNode::start(&node_config).await;
    }

    Ok(())
}
