use chrono::{DateTime, Duration, Utc};
use rmp_serde::{decode, encode};
use serde::{Deserialize, Serialize};

/// Message sent from the dialling peer to initiate an RTT measurement.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PingMessage {
    pub sequence: u32,
    pub sent_at: DateTime<Utc>,
}

impl PingMessage {
    pub fn new(sequence: u32, sent_at: DateTime<Utc>) -> Self {
        Self { sequence, sent_at }
    }
}

/// Response to a [`PingMessage`] that includes timing metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PongMessage {
    pub sequence: u32,
    pub sent_at: DateTime<Utc>,
    pub received_ping_at: DateTime<Utc>,
}

impl PongMessage {
    pub fn new(sequence: u32, received_ping_at: DateTime<Utc>, sent_at: DateTime<Utc>) -> Self {
        Self {
            sequence,
            sent_at,
            received_ping_at,
        }
    }
}

/// Aggregated round-trip timing data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RttReport {
    pub sequence: u32,
    pub rtt_ms: f64,
    pub attempts: u8,
}

impl RttReport {
    pub fn from_messages(ping: &PingMessage, pong: &PongMessage, attempts: u8) -> Self {
        let delta = pong.sent_at - ping.sent_at;
        let rtt_ms = duration_to_millis(delta).unwrap_or_default();
        Self {
            sequence: ping.sequence,
            rtt_ms,
            attempts,
        }
    }
}

fn duration_to_millis(duration: Duration) -> Option<f64> {
    duration.num_microseconds().map(|micro| micro as f64 / 1000.0)
}

/// Encode a [`PingMessage`] using MessagePack.
pub fn encode_ping(message: &PingMessage) -> Result<Vec<u8>, encode::Error> {
    encode::to_vec_named(message)
}

/// Decode a [`PingMessage`] from MessagePack bytes.
pub fn decode_ping(bytes: &[u8]) -> Result<PingMessage, decode::Error> {
    decode::from_slice(bytes)
}

/// Encode a [`PongMessage`] using MessagePack.
pub fn encode_pong(message: &PongMessage) -> Result<Vec<u8>, encode::Error> {
    encode::to_vec_named(message)
}

/// Decode a [`PongMessage`] from MessagePack bytes.
pub fn decode_pong(bytes: &[u8]) -> Result<PongMessage, decode::Error> {
    decode::from_slice(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_ping_roundtrip() {
        let ping = PingMessage::new(1, Utc::now());
        let bytes = encode_ping(&ping).expect("encode");
        let decoded = decode_ping(&bytes).expect("decode");
        assert_eq!(ping.sequence, decoded.sequence);
    }

    #[test]
    fn encode_decode_pong_roundtrip() {
        let ping = PingMessage::new(42, Utc::now());
        let pong = PongMessage::new(ping.sequence, ping.sent_at, Utc::now());
        let bytes = encode_pong(&pong).expect("encode");
        let decoded = decode_pong(&bytes).expect("decode");
        assert_eq!(pong.sequence, decoded.sequence);
    }

    #[test]
    fn rtt_report_computes_millis() {
        let sent_at = Utc::now();
        let ping = PingMessage::new(7, sent_at);
        let pong_time = sent_at + Duration::milliseconds(125);
        let pong = PongMessage::new(ping.sequence, sent_at, pong_time);
        let report = RttReport::from_messages(&ping, &pong, 1);
        assert_eq!(report.sequence, 7);
        assert!((report.rtt_ms - 125.0).abs() < f64::EPSILON);
    }
}
