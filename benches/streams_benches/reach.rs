use crate::run_setup;
use criterion::Criterion;
use pubky_nexus::models::post::{PostStream, PostStreamSorting, ViewerStreamSource};
use pubky_nexus::routes::v0::stream::utils::{PostStreamFilters, PostStreamValues};
use tokio::runtime::Runtime;

/// REACH RELATED POST STREAMS BENCHMARKS
pub fn bench_stream_followers_timeline(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the post streams with reach 'Followers' sorting 'Timeline'.");
    println!("***************************************");

    run_setup();

    let viewer_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_followers", |b| {
        b.to_async(&rt).iter(|| async {
            // Define all the arguments of the post stream
            let post_stream_values_with_viewer =
                PostStreamValues::new(Some(viewer_id.to_string()), None, None, None);
            let post_stream_filter = PostStreamFilters::new(
                PostStreamSorting::Timeline,
                ViewerStreamSource::Following,
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

pub fn bench_stream_following_timeline(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the post streams with reach 'Following' sorting 'Timeline'.");
    println!("***************************************");

    run_setup();

    let viewer_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_following", |b| {
        b.to_async(&rt).iter(|| async {
            // Define all the arguments of the post stream
            let post_stream_values_with_viewer =
                PostStreamValues::new(Some(viewer_id.to_string()), None, None, None);
            let post_stream_filter = PostStreamFilters::new(
                PostStreamSorting::Timeline,
                ViewerStreamSource::Followers,
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

pub fn bench_stream_friends_timeline(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the post streams with reach 'Friends' sorting 'Timeline'.");
    println!("***************************************");

    run_setup();

    let viewer_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_friends", |b| {
        b.to_async(&rt).iter(|| async {
            // Define all the arguments of the post stream
            let post_stream_values_with_viewer =
                PostStreamValues::new(Some(viewer_id.to_string()), None, None, None);
            let post_stream_filter = PostStreamFilters::new(
                PostStreamSorting::Timeline,
                ViewerStreamSource::Friends,
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

pub fn bench_stream_followers_total_engagement(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the post streams with reach 'Followers' sorting 'TotalEngagement'.");
    println!("***************************************");

    run_setup();

    let viewer_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_followers_total_engagement", |b| {
        b.to_async(&rt).iter(|| async {
            // Define all the arguments of the post stream
            let post_stream_values_with_viewer =
                PostStreamValues::new(Some(viewer_id.to_string()), None, None, None);
            let post_stream_filter = PostStreamFilters::new(
                PostStreamSorting::TotalEngagement,
                ViewerStreamSource::Followers,
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

pub fn bench_stream_following_total_engagement(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the post streams with reach 'Following' sorting 'TotalEngagement'.");
    println!("***************************************");

    run_setup();

    let viewer_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_following_total_engagement", |b| {
        b.to_async(&rt).iter(|| async {
            // Define all the arguments of the post stream
            let post_stream_values_with_viewer =
                PostStreamValues::new(Some(viewer_id.to_string()), None, None, None);
            let post_stream_filter = PostStreamFilters::new(
                PostStreamSorting::TotalEngagement,
                ViewerStreamSource::Following,
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

pub fn bench_stream_friends_total_engagement(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the post streams with reach 'Friends' sorting 'TotalEngagement'.");
    println!("***************************************");

    run_setup();

    let viewer_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_friends_total_engagement", |b| {
        b.to_async(&rt).iter(|| async {
            // Define all the arguments of the post stream
            let post_stream_values_with_viewer =
                PostStreamValues::new(Some(viewer_id.to_string()), None, None, None);
            let post_stream_filter = PostStreamFilters::new(
                PostStreamSorting::TotalEngagement,
                ViewerStreamSource::Friends,
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
