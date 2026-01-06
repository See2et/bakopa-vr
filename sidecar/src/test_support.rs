use std::sync::Mutex;

use syncer::TransportSendParams;

static LAST_SEND_PARAMS: Mutex<Option<TransportSendParams>> = Mutex::new(None);

pub fn record_send_params(params: TransportSendParams) {
    let mut guard = LAST_SEND_PARAMS.lock().expect("lock send params");
    *guard = Some(params);
}

pub fn last_send_params() -> Option<TransportSendParams> {
    LAST_SEND_PARAMS.lock().expect("lock send params").clone()
}
