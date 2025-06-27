use crate::{run_setup, streams_benches::LIMIT_20};
use criterion::Criterion;
use nexus_common::{
    db::kv::SortOrder,
    models::post::{PostStream, StreamSource},
    types::StreamSorting,
};
use tokio::runtime::Runtime;

const AUTHOR_ID: &str = "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy";

/// AUTHOR RELATED POST STREAMS BENCHMARKS
pub fn bench_stream_author_timeline(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Benchmarking the post streams for an Author.");
    println!("******************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_author", |b| {
        b.to_async(&rt).iter(|| async {
            // Define all the arguments of the post stream
            let source = StreamSource::Author {
                author_id: AUTHOR_ID.to_string(),
            };

            // Run the benchmark
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

pub fn bench_stream_author_total_engagement(c: &mut Criterion) {
    println!("************************************************************************");
    println!("Benchmarking the post streams an Author sorted by 'TotalEngagement'.");
    println!("************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_author_total_engagement", |b| {
        b.to_async(&rt).iter(|| async {
            // Define all the arguments of the post stream
            let source = StreamSource::Author {
                author_id: AUTHOR_ID.to_string(),
            };

            // Run the benchmark
            let post_stream = PostStream::get_posts(
                source,
                LIMIT_20,
                SortOrder::Descending,
                StreamSorting::TotalEngagement,
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

pub fn bench_stream_author_replies_timeline(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Benchmarking the post stream of replies of an Author.");
    println!("******************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_author_replies", |b| {
        b.to_async(&rt).iter(|| async {
            // Define all the arguments of the post stream
            let source = StreamSource::AuthorReplies {
                author_id: AUTHOR_ID.to_string(),
            };

            // Run the benchmark
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
