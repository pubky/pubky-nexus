use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use pubky_nexus::models::post::{PostStream, PostStreamSorting};
use pubky_nexus::models::user::{UserStream, UserStreamType};
use pubky_nexus::setup;
use pubky_nexus::Config;
use std::env;
use std::sync::Once;
use std::time::Duration;
use tokio::runtime::Runtime;

static INIT: Once = Once::new();

pub fn run_setup() {
    INIT.call_once(|| {
        let rt = Runtime::new().unwrap();
        env::set_var("RUST_LOG", "error");
        rt.block_on(async {
            let config = Config::from_env();
            setup(&config).await;
        });
    });
}

fn bench_stream_followers(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the user streams for a user's followers.");
    println!("***************************************");

    run_setup();

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("stream_followers", user_id),
        &user_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let user_stream =
                    UserStream::get_by_id(id, None, None, None, UserStreamType::Followers)
                        .await
                        .unwrap();
                criterion::black_box(user_stream);
            });
        },
    );
}

fn bench_stream_following(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the user streams for a user's followers.");
    println!("***************************************");

    run_setup();

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("stream_following", user_id),
        &user_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let user_stream =
                    UserStream::get_by_id(id, None, None, None, UserStreamType::Following)
                        .await
                        .unwrap();
                criterion::black_box(user_stream);
            });
        },
    );
}

fn bench_stream_posts_timeline(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the post streams sorted by timeline.");
    println!("***************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_timeline", |b| {
        b.to_async(&rt).iter(|| async {
            let post_stream =
                PostStream::get_sorted_posts(PostStreamSorting::Recency, None, None, Some(10))
                    .await
                    .unwrap();
            criterion::black_box(post_stream);
        });
    });
}

fn bench_stream_posts_total_engagement(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the post streams sorted by total engagement.");
    println!("***************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_total_engagement", |b| {
        b.to_async(&rt).iter(|| async {
            let post_stream = PostStream::get_sorted_posts(
                PostStreamSorting::TotalEngagement,
                None,
                None,
                Some(10),
            )
            .await
            .unwrap();
            criterion::black_box(post_stream);
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
    targets = bench_stream_followers,
              bench_stream_following,
              bench_stream_posts_timeline,
              bench_stream_posts_total_engagement,
}

criterion_main!(benches);
