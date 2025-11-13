use crate::{run_setup, streams_benches::LIMIT_20};
use criterion::Criterion;
use nexus_common::db::kv::SortOrder;
use nexus_common::models::post::{PostStream, StreamSource};
use nexus_common::types::StreamSorting;
use tokio::runtime::Runtime;

/// POST KEY STREAM BENCHMARKS
pub fn bench_stream_post_keys_all_timeline(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Benchmarking the post key stream with reach 'All' sorting 'Timeline'.");
    println!("******************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("stream_post_keys_all_timeline", |b| {
        b.to_async(&rt).iter(|| async {
            let source = StreamSource::All;

            let post_key_stream = PostStream::get_post_keys(
                source,
                LIMIT_20,
                SortOrder::Descending,
                StreamSorting::Timeline,
                None,
                None,
            )
            .await
            .unwrap()
            .expect("expected post keys in benchmark");

            std::hint::black_box((&post_key_stream.post_keys, post_key_stream.last_post_score));
        });
    });
}
