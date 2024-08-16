use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use pubky_nexus::models::post::{PostStream, PostStreamReach, PostStreamSorting};
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
                PostStream::get_global_posts(PostStreamSorting::Timeline, None, None, Some(10))
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
            let post_stream = PostStream::get_global_posts(
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

fn bench_stream_most_followed(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the user streams for most followed users.");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("stream_most_followed", |b| {
        b.to_async(&rt).iter(|| async {
            let user_stream =
                UserStream::get_by_id("", None, None, Some(10), UserStreamType::MostFollowed)
                    .await
                    .unwrap();
            criterion::black_box(user_stream);
        });
    });
}

fn bench_stream_user_posts(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the post streams for a specific user.");
    println!("***************************************");

    run_setup();

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("stream_user_posts", user_id),
        &user_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let post_stream = PostStream::get_user_posts(id, None, None, Some(10))
                    .await
                    .unwrap();
                criterion::black_box(post_stream);
            });
        },
    );
}

fn bench_stream_posts_following_reach(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the post streams with reach 'Following'.");
    println!("***************************************");

    run_setup();

    let viewer_id = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"; // Replace with an actual viewer ID for testing
    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_following_reach", |b| {
        b.to_async(&rt).iter(|| async {
            let post_stream = PostStream::get_posts_by_reach(
                PostStreamReach::Following,
                Some(viewer_id.to_string()),
                None,
                Some(10),
            )
            .await
            .unwrap();
            criterion::black_box(post_stream);
        });
    });
}

fn bench_stream_posts_followers_reach(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the post streams with reach 'Followers'.");
    println!("***************************************");

    run_setup();

    let viewer_id = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"; // Replace with an actual viewer ID for testing
    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_followers_reach", |b| {
        b.to_async(&rt).iter(|| async {
            let post_stream = PostStream::get_posts_by_reach(
                PostStreamReach::Followers,
                Some(viewer_id.to_string()),
                None,
                Some(10),
            )
            .await
            .unwrap();
            criterion::black_box(post_stream);
        });
    });
}

fn bench_stream_posts_friends_reach(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the post streams with reach 'Friends'.");
    println!("***************************************");

    run_setup();

    let viewer_id = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"; // Replace with an actual viewer ID for testing
    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_friends_reach", |b| {
        b.to_async(&rt).iter(|| async {
            let post_stream = PostStream::get_posts_by_reach(
                PostStreamReach::Friends,
                Some(viewer_id.to_string()),
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
              bench_stream_most_followed,
              bench_stream_user_posts,
              bench_stream_posts_following_reach,
              bench_stream_posts_followers_reach,
              bench_stream_posts_friends_reach,
}

criterion_main!(benches);
