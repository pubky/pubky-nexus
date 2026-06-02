use crate::run_setup;
use crate::streams_benches::LIMIT_20;
use criterion::Criterion;
use nexus_common::db::kv::SortOrder;
use nexus_common::models::post::{PostStream, StreamSource};
use nexus_common::types::{StreamSorting, WotDepth};
use tokio::runtime::Runtime;

// Real, high-degree observer from the seeded skunk graph (same as the reach benches).
const OBSERVER_ID: &str = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";

/// WEB OF TRUST POST STREAM BENCHMARKS
fn run_wot_depth(depth: u8, label: &str, c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Benchmarking the post streams with source 'Wot' depth {depth} sorting 'Timeline'.");
    println!("******************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function(label, |b| {
        b.to_async(&rt).iter(|| async {
            let source = StreamSource::Wot {
                observer_id: OBSERVER_ID.to_string(),
                depth: WotDepth::new(depth).expect("bench depth must be in range"),
            };

            let post_stream = PostStream::get_posts(
                source,
                LIMIT_20,
                SortOrder::Descending,
                StreamSorting::Timeline,
                None,
                None,
                None,
            )
            .await
            .unwrap();
            std::hint::black_box(post_stream);
        });
    });
}

pub fn bench_stream_wot_depth1_timeline(c: &mut Criterion) {
    run_wot_depth(1, "stream_posts_wot_depth1", c);
}

pub fn bench_stream_wot_depth2_timeline(c: &mut Criterion) {
    run_wot_depth(2, "stream_posts_wot_depth2", c);
}

pub fn bench_stream_wot_depth3_timeline(c: &mut Criterion) {
    run_wot_depth(3, "stream_posts_wot_depth3", c);
}

pub fn bench_stream_wot_domain_depth2(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Benchmarking the post streams with source 'WotDomain' depth 2 sorting 'Timeline'.");
    println!("******************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_wot_domain_depth2", |b| {
        b.to_async(&rt).iter(|| async {
            let source = StreamSource::WotDomain {
                observer_id: OBSERVER_ID.to_string(),
                depth: WotDepth::DEFAULT,
                domain_tags: vec!["bitcoin".to_string(), "dev".to_string()],
            };

            let post_stream = PostStream::get_posts(
                source,
                LIMIT_20,
                SortOrder::Descending,
                StreamSorting::Timeline,
                None,
                None,
                None,
            )
            .await
            .unwrap();
            std::hint::black_box(post_stream);
        });
    });
}
