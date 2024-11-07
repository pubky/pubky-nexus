use crate::run_setup;
use crate::streams_benches::LIMIT_20;
use criterion::Criterion;
use pubky_nexus::models::post::PostStream;
use pubky_nexus::routes::v0::stream::queries::{Filters, StreamSource};
use pubky_nexus::routes::v0::stream::PostStreamQuery;
use pubky_nexus::types::StreamSorting;
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
            let post_stream = PostStream::get_posts(PostStreamQuery {
                source,
                sorting: Some(StreamSorting::Timeline),
                filters: Filters { tags: None },
                pagination: LIMIT_20,
                viewer_id: None,
            })
            .await
            .unwrap();
            criterion::black_box(post_stream);
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
            let post_stream = PostStream::get_posts(PostStreamQuery {
                source,
                sorting: Some(StreamSorting::TotalEngagement),
                filters: Filters { tags: None },
                pagination: LIMIT_20,
                viewer_id: None,
            })
            .await
            .unwrap();
            criterion::black_box(post_stream);
        });
    });
}
