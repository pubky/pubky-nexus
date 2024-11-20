use crate::run_setup;
use crate::streams_benches::LIMIT_20;
use criterion::Criterion;
use pubky_nexus::models::post::{PostStream, StreamSource};
use pubky_nexus::models::pubky_app::PostKind;
use pubky_nexus::types::StreamSorting;
use tokio::runtime::Runtime;

/// POST KIND RELATED STREAMS BENCHMARKS
pub fn bench_stream_post_kind_short(c: &mut Criterion) {
    println!("***************************************************************************");
    println!("Benchmarking the post streams with kind 'short'");
    println!("***************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_kind_short", |b| {
        b.to_async(&rt).iter(|| async {
            // Define all the arguments of the post stream
            let source = StreamSource::All;

            // Run the benchmark
            let post_stream = PostStream::get_posts(
                source,
                LIMIT_20,
                StreamSorting::Timeline,
                None,
                None,
                Some(PostKind::Short),
            )
            .await
            .unwrap();
            criterion::black_box(post_stream);
        });
    });
}

pub fn bench_stream_post_kind_long(c: &mut Criterion) {
    println!("***************************************************************************");
    println!("Benchmarking the post streams with kind 'long'");
    println!("***************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_kind_long", |b| {
        b.to_async(&rt).iter(|| async {
            // Define all the arguments of the post stream
            let source = StreamSource::All;

            // Run the benchmark
            let post_stream = PostStream::get_posts(
                source,
                LIMIT_20,
                StreamSorting::Timeline,
                None,
                None,
                Some(PostKind::Long),
            )
            .await
            .unwrap();
            criterion::black_box(post_stream);
        });
    });
}

pub fn bench_stream_post_kind_image(c: &mut Criterion) {
    println!("***************************************************************************");
    println!("Benchmarking the post streams with kind 'image'");
    println!("***************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_kind_image", |b| {
        b.to_async(&rt).iter(|| async {
            // Define all the arguments of the post stream
            let source = StreamSource::All;

            // Run the benchmark
            let post_stream = PostStream::get_posts(
                source,
                LIMIT_20,
                StreamSorting::Timeline,
                None,
                None,
                Some(PostKind::Image),
            )
            .await
            .unwrap();
            criterion::black_box(post_stream);
        });
    });
}

pub fn bench_stream_post_kind_video(c: &mut Criterion) {
    println!("***************************************************************************");
    println!("Benchmarking the post streams with kind 'video'");
    println!("***************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_kind_video", |b| {
        b.to_async(&rt).iter(|| async {
            // Define all the arguments of the post stream
            let source = StreamSource::All;

            // Run the benchmark
            let post_stream = PostStream::get_posts(
                source,
                LIMIT_20,
                StreamSorting::Timeline,
                None,
                None,
                Some(PostKind::Video),
            )
            .await
            .unwrap();
            criterion::black_box(post_stream);
        });
    });
}

pub fn bench_stream_post_kind_link(c: &mut Criterion) {
    println!("***************************************************************************");
    println!("Benchmarking the post streams with kind 'link'");
    println!("***************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_kind_link", |b| {
        b.to_async(&rt).iter(|| async {
            // Define all the arguments of the post stream
            let source = StreamSource::All;

            // Run the benchmark
            let post_stream = PostStream::get_posts(
                source,
                LIMIT_20,
                StreamSorting::Timeline,
                None,
                None,
                Some(PostKind::Link),
            )
            .await
            .unwrap();
            criterion::black_box(post_stream);
        });
    });
}

pub fn bench_stream_post_kind_file(c: &mut Criterion) {
    println!("***************************************************************************");
    println!("Benchmarking the post streams with kind 'file'");
    println!("***************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_kind_file", |b| {
        b.to_async(&rt).iter(|| async {
            // Define all the arguments of the post stream
            let source = StreamSource::All;

            // Run the benchmark
            let post_stream = PostStream::get_posts(
                source,
                LIMIT_20,
                StreamSorting::Timeline,
                None,
                None,
                Some(PostKind::File),
            )
            .await
            .unwrap();
            criterion::black_box(post_stream);
        });
    });
}
