//! OpenTelemetry instruments for Web-of-Trust post streams (spec v3.1 "Metrics").
//!
//! Mirrors the `GraphMetrics` pattern in `db::graph::instrumented`: instruments
//! are built from the global meter and are no-ops when no `SdkMeterProvider` is
//! registered (i.e. when OTLP is not configured), so there is zero overhead in
//! that case.
//!
//! Mapping from these (dotted) instrument names to the spec's metric names. The
//! `source` and `depth` attributes carry the spec's `{source}`/`{depth}`
//! dimensions, so `wot` and `wot_domain` share one instrument rather than
//! splitting into two metric names (the idiomatic low-cardinality OTel shape):
//!
//! | Instrument                         | Spec metric                                |
//! |------------------------------------|--------------------------------------------|
//! | `wot.post_stream.requests{source}` | `wot_post_stream_requests{source=wot}` and `wot_domain_post_stream_requests{source=wot_domain}` |
//! | `wot.post_stream.empty` / requests | `wot_feed_empty_rate{depth}`               |
//! | `wot.post_stream.query_duration`   | `wot_query_latency_p95{depth}` (p95 derived downstream) |
//! | `wot.post_stream.returned_posts`   | adoption/load proxy — see below            |
//!
//! `wot_membership_size_p95{depth}` (spec; >10k → cap depth) is NOT emitted: the
//! true trusted-member count needs its own traversal, which belongs with the
//! deferred WoT cache work (a spec non-goal here). `returned_posts` is the cheap
//! stand-in available from the query we already run.

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
    /// Posts returned by the WoT query. A cheap load proxy; NOT the spec's
    /// trust-membership size (which needs its own traversal — see module docs).
    returned_posts: Histogram<u64>,
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
            returned_posts: meter
                .u64_histogram("wot.post_stream.returned_posts")
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
    METRICS.returned_posts.record(posts as u64, attrs);
    if posts == 0 {
        METRICS.empty.add(1, attrs);
    }
}
