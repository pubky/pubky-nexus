use crate::run_setup;
use crate::streams_benches::LIMIT_20;
use criterion::Criterion;
use nexus_common::db::kv::SortOrder;
use nexus_common::models::post::{PostStream, StreamSource};
use nexus_common::types::StreamSorting;
use tokio::runtime::Runtime;

const TAG: &str = "free";

/// TAG RELATED POST STREAMS BENCHMARKS
pub fn bench_stream_tag_timeline(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Benchmarking the post streams with tag 'free' sorted by 'Timeline'.");
    println!("******************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_tag_timeline", |b| {
        b.to_async(&rt).iter(|| async {
            // Define all the arguments of the post stream
            let source = StreamSource::All;

            // Run the benchmark
            let post_stream = PostStream::get_posts(
                source,
                LIMIT_20,
                SortOrder::Descending,
                StreamSorting::Timeline,
                None,
                Some(vec![TAG.to_string()]),
                None,
            )
            .await
            .unwrap();
            std::hint::black_box(post_stream);
        });
    });
}

pub fn bench_stream_tag_total_engagement(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Benchmarking the post streams with tag 'free' sorted by 'TotalEngagement'.");
    println!("******************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_tag_total_engagement", |b| {
        b.to_async(&rt).iter(|| async {
            // Define all the arguments of the post stream
            let source = StreamSource::All;

            // Run the benchmark
            let post_stream = PostStream::get_posts(
                source,
                LIMIT_20,
                SortOrder::Descending,
                StreamSorting::TotalEngagement,
                None,
                Some(vec![TAG.to_string()]),
                None,
            )
            .await
            .unwrap();
            std::hint::black_box(post_stream);
        });
    });
}
