use crate::run_setup;
use criterion::Criterion;
use pubky_nexus::models::post::{PostStream, PostStreamSorting, ViewerStreamSource};
use pubky_nexus::routes::v0::stream::utils::{PostStreamFilters, PostStreamValues};
use tokio::runtime::Runtime;

/// BOOKMARK RELATED POST STREAMS BENCHMARKS
pub fn bench_stream_bookmarks_timeline(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the post streams with reach 'Bookmarks' sorting 'Timeline'.");
    println!("***************************************");

    run_setup();

    let viewer_id = "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy";
    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_bookmarks_timeline", |b| {
        b.to_async(&rt).iter(|| async {
            // Define all the arguments of the post stream
            let post_stream_values_with_viewer =
                PostStreamValues::new(Some(viewer_id.to_string()), None, None, None);
            let post_stream_filter = PostStreamFilters::new(
                PostStreamSorting::Timeline,
                ViewerStreamSource::Bookmarks,
                None,
                Some(20),
                None,
                None,
            );
            // Run the benchmark
            let post_stream =
                PostStream::get_posts(post_stream_values_with_viewer, post_stream_filter)
                    .await
                    .unwrap();
            criterion::black_box(post_stream);
        });
    });
}

pub fn bench_stream_bookmarks_total_engagement(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the post streams with reach 'Bookmarks' sorting 'TotalEngagement'.");
    println!("***************************************");

    run_setup();

    let viewer_id = "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy";
    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_bookmarks_total_engagement", |b| {
        b.to_async(&rt).iter(|| async {
            // Define all the arguments of the post stream
            let post_stream_values_with_viewer =
                PostStreamValues::new(Some(viewer_id.to_string()), None, None, None);
            let post_stream_filter = PostStreamFilters::new(
                PostStreamSorting::TotalEngagement,
                ViewerStreamSource::Bookmarks,
                None,
                Some(20),
                None,
                None,
            );
            // Run the benchmark
            let post_stream =
                PostStream::get_posts(post_stream_values_with_viewer, post_stream_filter)
                    .await
                    .unwrap();
            criterion::black_box(post_stream);
        });
    });
}
