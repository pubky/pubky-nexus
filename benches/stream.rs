use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use pubky_nexus::models::post::{PostStream, PostStreamReach, PostStreamSorting};
use pubky_nexus::models::user::{UserStream, UserStreamType};
use setup::run_setup;
use std::time::Duration;
use tokio::runtime::Runtime;

mod setup;

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
    println!("***************************************");

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

fn bench_stream_pioneers(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the user streams for pioneer users.");
    println!("***************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("stream_pioneers", |b| {
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

fn bench_stream_post_tag_timeline(c: &mut Criterion) {
    println!("****************************************************************");
    println!("Benchmarking the post stream filtered by tag sort by timeline");
    println!("****************************************************************");

    run_setup();

    let label = "free";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("tag_search_by_timeline", label),
        &label,
        |b, &label| {
            b.to_async(&rt).iter(|| async {
                let result = PostStream::get_posts_by_tag(label, None, None, None, None)
                    .await
                    .unwrap();
                criterion::black_box(result);
            });
        },
    );
}

fn bench_stream_post_tag_engagement(c: &mut Criterion) {
    println!("****************************************************************");
    println!("Benchmarking the post stream filtered by tag sort by engagement");
    println!("****************************************************************");

    run_setup();

    let label = "free";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("tag_search_by_engagement", label),
        &label,
        |b, &label| {
            b.to_async(&rt).iter(|| async {
                let result = PostStream::get_posts_by_tag(
                    label,
                    Some(PostStreamSorting::TotalEngagement),
                    None,
                    None,
                    None,
                )
                .await
                .unwrap();
                criterion::black_box(result);
            });
        },
    );
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
              bench_stream_pioneers,
              bench_stream_user_posts,
              bench_stream_posts_following_reach,
              bench_stream_posts_followers_reach,
              bench_stream_posts_friends_reach,
              bench_stream_users_by_username_search,
              bench_stream_post_tag_timeline,
              bench_stream_post_tag_engagement
}

criterion_main!(benches);
