use crate::run_setup;
use crate::streams_benches::LIMIT_20;
use criterion::Criterion;
use nexus_common::db::kv::SortOrder;
use nexus_common::models::post::{PostStream, StreamSource};
use nexus_common::types::StreamSorting;
use tokio::runtime::Runtime;

const OBSERVER_ID: &str = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";

/// REACH RELATED POST STREAMS BENCHMARKS
pub fn bench_stream_followers_timeline(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Benchmarking the post streams with reach 'Followers' sorting 'Timeline'.");
    println!("******************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_followers", |b| {
        b.to_async(&rt).iter(|| async {
            // Define all the arguments of the post stream
            let source = StreamSource::Followers {
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

pub fn bench_stream_following_timeline(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Benchmarking the post streams with reach 'Following' sorting 'Timeline'.");
    println!("******************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_following", |b| {
        b.to_async(&rt).iter(|| async {
            // Define all the arguments of the post stream
            let source = StreamSource::Following {
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

pub fn bench_stream_friends_timeline(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Benchmarking the post streams with reach 'Friends' sorting 'Timeline'.");
    println!("******************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_friends", |b| {
        b.to_async(&rt).iter(|| async {
            // Define all the arguments of the post stream
            let source = StreamSource::Friends {
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

pub fn bench_stream_followers_total_engagement(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Benchmarking the post streams with reach 'Followers' sorting 'TotalEngagement'.");
    println!("******************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_followers_total_engagement", |b| {
        b.to_async(&rt).iter(|| async {
            // Define all the arguments of the post stream
            let source = StreamSource::Followers {
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

pub fn bench_stream_following_total_engagement(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Benchmarking the post streams with reach 'Following' sorting 'TotalEngagement'.");
    println!("******************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_following_total_engagement", |b| {
        b.to_async(&rt).iter(|| async {
            // Define all the arguments of the post stream
            let source = StreamSource::Following {
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

pub fn bench_stream_friends_total_engagement(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Benchmarking the post streams with reach 'Friends' sorting 'TotalEngagement'.");
    println!("******************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_friends_total_engagement", |b| {
        b.to_async(&rt).iter(|| async {
            // Define all the arguments of the post stream
            let source = StreamSource::Friends {
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
