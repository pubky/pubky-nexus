use crate::run_setup;
use crate::streams_benches::LIMIT_20;
use criterion::Criterion;
use pubky_nexus::models::post::PostStream;
use pubky_nexus::routes::v0::stream::queries::{Filters, StreamSource};
use pubky_nexus::routes::v0::stream::PostStreamQuery;
use pubky_nexus::types::StreamSorting;
use tokio::runtime::Runtime;

const TAG: &str = "free";

/// TAG RELATED POST STREAMS BENCHMARKS
pub fn bench_stream_tag_timeline(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the post streams with tag 'free' sorted by 'Timeline'.");
    println!("***************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_tag_timeline", |b| {
        b.to_async(&rt).iter(|| async {
            // Define all the arguments of the post stream
            let source = StreamSource::All {
                author_id: None,
            };

            // Run the benchmark
            let post_stream = PostStream::get_posts(PostStreamQuery {
                source,
                sorting: Some(StreamSorting::Timeline),
                filters: Filters { tags: Some(vec![TAG.to_string()]) },
                pagination: LIMIT_20,
                viewer_id: None,
            })
            .await
            .unwrap();
            criterion::black_box(post_stream);
        });
    });
}

pub fn bench_stream_tag_total_engagement(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the post streams with tag 'free' sorted by 'TotalEngagement'.");
    println!("***************************************");

    run_setup();
    
    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_tag_total_engagement", |b| {
        b.to_async(&rt).iter(|| async {
            // Define all the arguments of the post stream
            let source = StreamSource::All {
                author_id: None,
            };

            // Run the benchmark
            let post_stream = PostStream::get_posts(PostStreamQuery {
                source,
                sorting: Some(StreamSorting::TotalEngagement),
                filters: Filters { tags: Some(vec![TAG.to_string()]) },
                pagination: LIMIT_20,
                viewer_id: None,
            })
            .await
            .unwrap();
            criterion::black_box(post_stream);
        });
    });
}
