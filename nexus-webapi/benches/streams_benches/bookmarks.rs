use crate::run_setup;
use crate::streams_benches::LIMIT_20;
use criterion::{BenchmarkId, Criterion};
use nexus_common::db::kv::SortOrder;
use nexus_common::models::post::{PostStream, StreamSource};
use nexus_common::types::StreamSorting;
use tokio::runtime::Runtime;

const OBSERVER_ID: &str = "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy";

/// BOOKMARK RELATED POST STREAMS BENCHMARKS
pub fn bench_stream_bookmarks_timeline(c: &mut Criterion) {
    println!("***************************************************************************");
    println!("Benchmarking the post streams with reach 'Bookmarks' sorting 'Timeline'.");
    println!("***************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_bookmarks_timeline", |b| {
        b.to_async(&rt).iter(|| async {
            // Define all the arguments of the post stream
            let source = StreamSource::Bookmarks {
                observer_id: OBSERVER_ID.to_string(),
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

pub fn bench_stream_bookmarks_total_engagement(c: &mut Criterion) {
    println!("*********************************************************************************");
    println!("Benchmarking the post streams with reach 'Bookmarks' sorting 'TotalEngagement'.");
    println!("*********************************************************************************");

    run_setup();
    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_bookmarks_total_engagement", |b| {
        b.to_async(&rt).iter(|| async {
            // Define all the arguments of the post stream
            let source = StreamSource::Bookmarks {
                observer_id: OBSERVER_ID.to_string(),
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

/// A page of the bookmarked post keys of `OBSERVER_ID`, the mock user with the
/// most bookmarks (15 posts over 37 edges): newest first, and ascending
/// between two bookmark times (11 posts).
pub fn bench_stream_bookmarked_post_keys(c: &mut Criterion) {
    println!("***************************************************************************");
    println!(
        "Benchmarking a page of the bookmarked post keys of the user with the most bookmarks."
    );
    println!("***************************************************************************");

    run_setup();
    let rt = Runtime::new().unwrap();

    let pages = [
        ("newest", SortOrder::Descending, None, None),
        (
            "ascending_bounded",
            SortOrder::Ascending,
            Some(1724134163100.0),
            Some(1724134131000.0),
        ),
    ];
    for (name, order, start, end) in pages {
        c.bench_with_input(
            BenchmarkId::new("stream_bookmarked_post_keys", name),
            &order,
            |b, order| {
                b.to_async(&rt).iter(|| async {
                    let keys = PostStream::get_bookmarked_posts(
                        OBSERVER_ID,
                        order.clone(),
                        start,
                        end,
                        None,
                        Some(20),
                    )
                    .await
                    .unwrap();
                    std::hint::black_box(keys);
                });
            },
        );
    }
}
