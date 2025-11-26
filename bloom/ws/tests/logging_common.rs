use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tracing::Subscriber;
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_subscriber::{
    layer::{Context, Layer},
    registry::{LookupSpan, Registry},
};

pub fn setup_tracing() -> (RecordingLayer, tracing::subscriber::DefaultGuard) {
    let layer = RecordingLayer::default();
    let subscriber = Registry::default().with(layer.clone());
    let guard = tracing::subscriber::set_default(subscriber);
    (layer, guard)
}

#[derive(Default, Clone)]
pub struct RecordingLayer {
    pub spans: Arc<Mutex<Vec<SpanRecord>>>,
}

#[derive(Default, Debug)]
pub struct SpanRecord {
    pub fields: HashMap<String, String>,
}

#[derive(Default)]
struct FieldVisitor {
    fields: HashMap<String, String>,
}

impl tracing::field::Visit for FieldVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.fields
            .insert(field.name().to_string(), format!("{value:?}"));
    }
}

impl<S> Layer<S> for RecordingLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        _id: &tracing::Id,
        _ctx: Context<'_, S>,
    ) {
        let mut visitor = FieldVisitor::default();
        attrs.record(&mut visitor);

        if let Ok(mut spans) = self.spans.lock() {
            spans.push(SpanRecord {
                fields: visitor.fields,
            });
        }
    }
}

pub fn spans_have_field_value(spans: &[SpanRecord], key: &str, expected: &str) -> bool {
    spans.iter().any(|s| {
        s.fields
            .get(key)
            .map(|v| v.contains(expected))
            .unwrap_or(false)
    })
}
