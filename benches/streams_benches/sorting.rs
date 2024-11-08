use crate::{run_setup, streams_benches::LIMIT_20};
use criterion::Criterion;
use pubky_nexus::{models::post::PostStream, routes::v0::stream::queries::StreamSource};
use pubky_nexus::types::StreamSorting;
use tokio::runtime::Runtime;

const AUTHOR_ID: &str = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";

/// SORTING RELATED POST STREAMS BENCHMARKS
pub fn bench_stream_all_timeline(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Benchmarking the post streams with reach 'All' sorting 'Timeline'.");
    println!("******************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_all_timeline", |b| {
        b.to_async(&rt).iter(|| async {
            // Define all the arguments of the post stream
            let source = StreamSource::All;

            // Run the benchmark
            let post_stream = PostStream::get_posts(
                source,
                LIMIT_20,
                StreamSorting::Timeline,
                None,
                None
            ) 
            .await
            .unwrap();
            criterion::black_box(post_stream);
        });
    });
}

pub fn bench_stream_all_total_engagement(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Benchmarking the post streams with reach 'All' sorting 'TotalEngagement'.");
    println!("******************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_all_total_engagement", |b| {
        b.to_async(&rt).iter(|| async {
            // Define all the arguments of the post stream
            let source = StreamSource::All;

            // Run the benchmark
            let post_stream = PostStream::get_posts(
                source,
                LIMIT_20,
                StreamSorting::TotalEngagement,
                None,
                None
            ) 
            .await
            .unwrap();
            criterion::black_box(post_stream);
        });
    });
}

pub fn bench_stream_author_timeline(c: &mut Criterion) {
    println!("*******************************************************************");
    println!("Benchmarking the post streams for author_id sorted by 'Timeline'.");
    println!("*******************************************************************");

    run_setup();

    
    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_author_timeline", |b| {
        b.to_async(&rt).iter(|| async {
            // Define all the arguments of the post stream
            let source = StreamSource::Author {
                author_id: Some(AUTHOR_ID.to_string()),
            };

            // Run the benchmark
            let post_stream = PostStream::get_posts(
                source,
                LIMIT_20,
                StreamSorting::Timeline,
                None,
                None
            ) 
            .await
            .unwrap();
            criterion::black_box(post_stream);
        });
    });
}

pub fn bench_stream_author_total_engagement(c: &mut Criterion) {
    println!("************************************************************************");
    println!("Benchmarking the post streams for author_id sorted by 'TotalEngagement'.");
    println!("************************************************************************");

    run_setup();
    
    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_author_total_engagement", |b| {
        b.to_async(&rt).iter(|| async {
            // Define all the arguments of the post stream
            let source = StreamSource::Author {
                author_id: Some(AUTHOR_ID.to_string()),
            };

            // Run the benchmark
            let post_stream = PostStream::get_posts(
                source,
                LIMIT_20,
                StreamSorting::TotalEngagement,
                None,
                None
            )
            .await
            .unwrap();
            criterion::black_box(post_stream);

            
        });
    });
}
