use std::sync::Mutex;

use std::sync::OnceLock;

use syncer::{SyncerEvent, TransportSendParams};
use tokio::sync::Notify;

static LAST_SEND_PARAMS: Mutex<Option<TransportSendParams>> = Mutex::new(None);
static INJECT_EVENTS: Mutex<Vec<SyncerEvent>> = Mutex::new(Vec::new());
static CLEARED_ROOM_ID: OnceLock<Mutex<Option<String>>> = OnceLock::new();
static CLEARED_NOTIFY: OnceLock<Notify> = OnceLock::new();
static LAST_TRANSPORT_KIND: OnceLock<Mutex<Option<String>>> = OnceLock::new();

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

fn cleared_room_slot() -> &'static Mutex<Option<String>> {
    CLEARED_ROOM_ID.get_or_init(|| Mutex::new(None))
}

fn cleared_notify() -> &'static Notify {
    CLEARED_NOTIFY.get_or_init(Notify::new)
}

pub fn record_cleared_room_id(room_id: String) {
    let mut guard = cleared_room_slot().lock().expect("lock cleared room id");
    *guard = Some(room_id);
    cleared_notify().notify_waiters();
}

pub async fn wait_for_cleared_room_id(timeout: std::time::Duration) -> Option<String> {
    let notified = tokio::time::timeout(timeout, cleared_notify().notified()).await;
    if notified.is_err() {
        return None;
    }
    cleared_room_slot().lock().expect("lock cleared room id").clone()
}

fn transport_kind_slot() -> &'static Mutex<Option<String>> {
    LAST_TRANSPORT_KIND.get_or_init(|| Mutex::new(None))
}

pub fn record_transport_kind(kind: &str) {
    let mut guard = transport_kind_slot().lock().expect("lock transport kind");
    *guard = Some(kind.to_string());
}

pub fn last_transport_kind() -> Option<String> {
    transport_kind_slot().lock().expect("lock transport kind").clone()
}
