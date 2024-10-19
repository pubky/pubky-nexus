use crate::run_setup;
use criterion::Criterion;
use pubky_nexus::models::post::{PostStream, PostStreamSorting, ViewerStreamSource};
use pubky_nexus::routes::v0::stream::utils::{PostStreamFilters, PostStreamValues};
use tokio::runtime::Runtime;

/// SORTING RELATED POST STREAMS BENCHMARKS
pub fn bench_stream_all_timeline(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the post streams with reach 'All' sorting 'Timeline'.");
    println!("***************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_all_timeline", |b| {
        b.to_async(&rt).iter(|| async {
            // Define all the arguments of the post stream
            let post_stream_values_with_viewer = PostStreamValues::new(None, None, None, None);
            let post_stream_filter = PostStreamFilters::new(
                PostStreamSorting::Timeline,
                ViewerStreamSource::All,
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

pub fn bench_stream_all_total_engagement(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the post streams with reach 'All' sorting 'TotalEngagement'.");
    println!("***************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_all_total_engagement", |b| {
        b.to_async(&rt).iter(|| async {
            // Define all the arguments of the post stream
            let post_stream_values_with_viewer = PostStreamValues::new(None, None, None, None);
            let post_stream_filter = PostStreamFilters::new(
                PostStreamSorting::TotalEngagement,
                ViewerStreamSource::All,
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

pub fn bench_stream_author_timeline(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the post streams for author_id sorted by 'Timeline'.");
    println!("***************************************");

    run_setup();

    let author_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_author_timeline", |b| {
        b.to_async(&rt).iter(|| async {
            // Define all the arguments of the post stream
            let post_stream_values_with_viewer =
                PostStreamValues::new(None, Some(author_id.to_string()), None, None);
            let post_stream_filter = PostStreamFilters::new(
                PostStreamSorting::Timeline,
                ViewerStreamSource::All,
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

pub fn bench_stream_author_total_engagement(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the post streams for author_id sorted by 'TotalEngagement'.");
    println!("***************************************");

    run_setup();

    let author_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_author_total_engagement", |b| {
        b.to_async(&rt).iter(|| async {
            // Define all the arguments of the post stream
            let post_stream_values_with_viewer =
                PostStreamValues::new(None, Some(author_id.to_string()), None, None);
            let post_stream_filter = PostStreamFilters::new(
                PostStreamSorting::TotalEngagement,
                ViewerStreamSource::All,
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
