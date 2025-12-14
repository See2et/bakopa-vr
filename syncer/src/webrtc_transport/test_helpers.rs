use std::sync::Arc;
use std::sync::Mutex;

use super::RealWebrtcTransport;
use crate::TransportSendParams;

/// テスト用: 生成された DataChannel のパラメータ履歴を取得
pub fn created_params(t: &RealWebrtcTransport) -> Vec<TransportSendParams> {
    t.created_params
        .lock()
        .map(|v| v.clone())
        .unwrap_or_default()
}
