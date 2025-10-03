use chrono::Utc;
use shared::{
    decode_ping, decode_pong, encode_ping, encode_pong, format_multiaddr, Keypair, PeerAddress,
    PingMessage, PongMessage,
};

#[test]
fn ping_and_pong_roundtrip() {
    let ping = PingMessage::new(1, Utc::now());
    let pong = PongMessage::new(ping.sequence, ping.sent_at, Utc::now());

    let encoded_ping = encode_ping(&ping).expect("encode ping");
    let encoded_pong = encode_pong(&pong).expect("encode pong");

    let decoded_ping = decode_ping(&encoded_ping).expect("decode ping");
    let decoded_pong = decode_pong(&encoded_pong).expect("decode pong");

    assert_eq!(decoded_ping.sequence, ping.sequence);
    assert_eq!(decoded_pong.sequence, pong.sequence);
    assert!(decoded_pong.sent_at >= decoded_ping.sent_at);
}

#[test]
fn multiaddr_format_and_parse() {
    let keypair = Keypair::generate();
    let listen_addr = "127.0.0.1:9000".parse().unwrap();
    let formatted = format_multiaddr(listen_addr, &keypair.peer_id());
    let peer_addr = PeerAddress::new(formatted);

    assert_eq!(peer_addr.peer_id().unwrap(), keypair.peer_id());
    assert_eq!(peer_addr.to_socket_addr().unwrap(), listen_addr);
}
