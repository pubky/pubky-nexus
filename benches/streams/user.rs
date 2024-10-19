use criterion::{criterion_group, criterion_main, Criterion};
use pubky_nexus::models::user::{UserStream, UserStreamSource};

use std::time::Duration;
use tokio::runtime::Runtime;
use crate::setup::run_setup;

/// USER STREAMS BENCHMARKS
fn bench_stream_following(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the user streams for following users.");
    println!("***************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";

    c.bench_function("stream_following", |b| {
        b.to_async(&rt).iter(|| async {
            let user_stream = UserStream::get_by_id(
                Some(user_id),
                None,
                None,
                Some(20),
                UserStreamSource::Pioneers,
            )
            .await
            .unwrap();
            criterion::black_box(user_stream);
        });
    });
}

fn bench_stream_most_followed(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the user streams for most followed users.");
    println!("***************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("stream_most_followed", |b| {
        b.to_async(&rt).iter(|| async {
            let user_stream =
                UserStream::get_by_id(None, None, None, Some(20), UserStreamSource::MostFollowed)
                    .await
                    .unwrap();
            criterion::black_box(user_stream);
        });
    });
}

fn bench_stream_users_by_username_search(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the user streams by username search.");
    println!("***************************************");

    run_setup();

    let username = "An"; // Match all anonymous profiles
    let rt = Runtime::new().unwrap();

    c.bench_function("stream_users_by_username_search", |b| {
        b.to_async(&rt).iter(|| async {
            let user_stream = UserStream::get_from_username_search(
                username,
                None,
                None,
                Some(40), // Limit to 40 results
            )
            .await
            .unwrap();
            criterion::black_box(user_stream);
        });
    });
}

fn bench_stream_pioneers(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the user streams for pioneer users.");
    println!("***************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("stream_pioneers", |b| {
        b.to_async(&rt).iter(|| async {
            let user_stream =
                UserStream::get_by_id(None, None, None, Some(20), UserStreamSource::Pioneers)
                    .await
                    .unwrap();
            criterion::black_box(user_stream);
        });
    });
}



fn configure_criterion() -> Criterion {
    Criterion::default()
        .measurement_time(Duration::new(5, 0))
        .sample_size(100)
        .warm_up_time(Duration::new(1, 0))
}

criterion_group! {
    name = benches;
    config = configure_criterion();
    targets = bench_stream_users_by_username_search,
              bench_stream_pioneers,
              bench_stream_following,
              bench_stream_most_followed,
}

criterion_main!(benches);