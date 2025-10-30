use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use syncer::{NodeConfig, SyncerNode};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    dbg!(&args);

    let pos = args.iter().position(|x| x == "--listen");
    let listen_addr = pos.and_then(|p| args.get(p + 1));

    if let Some(addr) = listen_addr {
        println!("listen = {}", addr);
    } else {
        eprintln!("--listen が指定されていない、または値がありません。");
    }

    let socket_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
    let node_config = NodeConfig::new(socket_addr);
    let _ = SyncerNode::start(&node_config).await;

    Ok(())
}
