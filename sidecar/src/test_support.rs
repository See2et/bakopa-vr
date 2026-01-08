use std::sync::Mutex;

use syncer::{SyncerEvent, TransportSendParams};

static LAST_SEND_PARAMS: Mutex<Option<TransportSendParams>> = Mutex::new(None);
static INJECT_EVENTS: Mutex<Vec<SyncerEvent>> = Mutex::new(Vec::new());

pub fn record_send_params(params: TransportSendParams) {
    let mut guard = LAST_SEND_PARAMS.lock().expect("lock send params");
    *guard = Some(params);
}

pub fn last_send_params() -> Option<TransportSendParams> {
    LAST_SEND_PARAMS.lock().expect("lock send params").clone()
}

pub fn push_injected_event(event: SyncerEvent) {
    let mut guard = INJECT_EVENTS.lock().expect("lock injected events");
    guard.push(event);
}

pub fn take_injected_events() -> Vec<SyncerEvent> {
    let mut guard = INJECT_EVENTS.lock().expect("lock injected events");
    let events = guard.clone();
    guard.clear();
    events
}
