use std::net::SocketAddr;
use std::time::Duration;

use syncer::{start_syncer, NodeConfig};

use tokio::time::timeout;

/// Ensures two Syncer nodes can bootstrap and exchange a pair of messages.
#[tokio::test(flavor = "multi_thread")]
async fn exchange_round_trip_messages_between_two_nodes() {
    let listen_a: SocketAddr = "127.0.0.1:47401".parse().expect("valid listen addr");
    let listen_b: SocketAddr = "127.0.0.1:47402".parse().expect("valid listen addr");

    let config_a = NodeConfig::new(listen_a).with_bootstrap_message("syncer-a");

    let handle_a = timeout(Duration::from_secs(10), start_syncer(config_a))
        .await
        .expect("node A bootstrap timed out")
        .expect("start_syncer for node A is not implemented yet");

    let peer_addr = handle_a.node().endpoint_addr();

    let config_b = NodeConfig::new(listen_b)
        .with_peer(peer_addr)
        .with_bootstrap_message("syncer-b");

    let handle_b = timeout(Duration::from_secs(10), start_syncer(config_b))
        .await
        .expect("node B bootstrap timed out")
        .expect("start_syncer for node B is not implemented yet");

    let channel_a = handle_a.channel().clone();
    let channel_b = handle_b.channel().clone();

    timeout(Duration::from_secs(5), async move {
        channel_a.send("ping from node A".to_owned()).await?;
        let received = channel_b.next().await?;
        assert_eq!(received, "ping from node A");

        channel_b.send("pong from node B".to_owned()).await?;
        let round_trip = channel_a.next().await?;
        assert_eq!(round_trip, "pong from node B");

        Ok::<_, anyhow::Error>(())
    })
    .await
    .expect("message exchange completes")
    .expect("message exchange not yet implemented");

    // TODO: Invoke handle shutdown once the API is implemented.
    // handle_a.shutdown().await.unwrap();
    // handle_b.shutdown().await.unwrap();
}
