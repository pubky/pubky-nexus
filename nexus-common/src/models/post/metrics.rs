//! OpenTelemetry instruments for Web-of-Trust post streams (spec v3.1 "Metrics").
//!
//! Mirrors the `GraphMetrics` pattern in `db::graph::instrumented`: instruments
//! are built from the global meter and are no-ops when no `SdkMeterProvider` is
//! registered (i.e. when OTLP is not configured), so there is zero overhead in
//! that case. Exported names map to `wot_post_stream_requests`,
//! `wot_post_stream_empty`, `wot_query_duration`, `wot_membership_size`.

use std::sync::LazyLock;
use std::time::Duration;

use opentelemetry::metrics::{Counter, Histogram};
use opentelemetry::{global, KeyValue};

const METER_NAME: &str = "wot";

struct WotStreamMetrics {
    /// One per `source=wot` / `source=wot_domain` request.
    requests: Counter<u64>,
    /// Requests that returned no posts (empty-rate = empty / requests).
    empty: Counter<u64>,
    /// Graph query wall-clock time, in milliseconds.
    query_duration: Histogram<f64>,
    /// Posts returned by the WoT query — a cheap proxy for trust-neighborhood
    /// load (the exact trusted-member count would need its own traversal).
    membership_size: Histogram<u64>,
}

impl WotStreamMetrics {
    fn new() -> Self {
        let meter = global::meter(METER_NAME);
        Self {
            requests: meter
                .u64_counter("wot.post_stream.requests")
                .with_description("Total WoT post-stream requests, by source")
                .build(),
            empty: meter
                .u64_counter("wot.post_stream.empty")
                .with_description("WoT post-stream requests returning no posts, by source/depth")
                .build(),
            query_duration: meter
                .f64_histogram("wot.post_stream.query_duration")
                .with_description("WoT post-stream graph query time, in milliseconds")
                .with_unit("ms")
                .build(),
            membership_size: meter
                .u64_histogram("wot.post_stream.membership_size")
                .with_description("Posts returned by a WoT post-stream query, by source/depth")
                .with_unit("{post}")
                .build(),
        }
    }
}

static METRICS: LazyLock<WotStreamMetrics> = LazyLock::new(WotStreamMetrics::new);

/// Count one WoT post-stream request. `source` is `"wot"` or `"wot_domain"`.
pub(super) fn record_wot_request(source: &'static str) {
    METRICS.requests.add(1, &[KeyValue::new("source", source)]);
}

/// Record a completed WoT query: latency, returned-post count, and emptiness.
pub(super) fn record_wot_result(source: &'static str, depth: u8, elapsed: Duration, posts: usize) {
    let attrs = &[
        KeyValue::new("source", source),
        KeyValue::new("depth", i64::from(depth)),
    ];
    METRICS
        .query_duration
        .record(elapsed.as_secs_f64() * 1000.0, attrs);
    METRICS.membership_size.record(posts as u64, attrs);
    if posts == 0 {
        METRICS.empty.add(1, attrs);
    }
}
