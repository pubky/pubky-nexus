use criterion::{criterion_group, criterion_main};
use criterion::{BenchmarkId, Criterion};
use pubky_nexus::models::tag::post::PostTags;
use pubky_nexus::models::tag::stream::HotTags;
use pubky_nexus::models::tag::user::UserTags;
use pubky_nexus::models::user::UserStreamType;
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

fn bench_get_user_tags(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Test the performance of getting a user tags, using index or graph as needed");
    println!("******************************************************************************");

    run_setup();

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("bench_get_user_tags", user_id),
        &user_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let profile = UserTags::get_by_id(id).await.unwrap();
                criterion::black_box(profile);
            });
        },
    );
}

fn bench_get_post_tags(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Test the performance of getting a post tags, using index or graph as needed");
    println!("******************************************************************************");

    run_setup();

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let post_id = "2Z1NJPW2QHGG0";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new(
            "bench_get_post_tags",
            format!("user_id: {}, post_id: {}", user_id, post_id),
        ),
        &[user_id, post_id],
        |b, &params| {
            b.to_async(&rt).iter(|| async {
                let profile = PostTags::get_by_id(params[0], params[1]).await.unwrap();
                criterion::black_box(profile);
            });
        },
    );
}

fn bench_get_global_hot_tags(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Test the performance of getting a global tags, using index or graph as needed");
    println!("******************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("bench_get_global_hot_tags", |b| {
        b.to_async(&rt).iter(|| async {
            let stream_tag = HotTags::get_global_tags_stream(None, Some(40))
                .await
                .unwrap();
            criterion::black_box(stream_tag);
        });
    });
}

fn bench_get_following_reach_hot_tags(c: &mut Criterion) {
    println!(
        "****************************************************************************************"
    );
    println!(
        "Test the performance of getting tags by following reach, using index or graph as needed"
    );
    println!(
        "****************************************************************************************"
    );

    run_setup();

    let user_id = "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo";
    let reach_by = format!("{:?}", UserStreamType::Following);
    let rt: Runtime = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new(
            "bench_get_user_following_hot_tags",
            format!("user_id: {}, reach: {}", user_id, reach_by),
        ),
        &[user_id],
        |b, &params| {
            b.to_async(&rt).iter(|| async {
                let profile = HotTags::get_stream_tags_by_reach(
                    String::from(params[0]),
                    UserStreamType::Following,
                )
                .await
                .unwrap();
                criterion::black_box(profile);
            });
        },
    );
}

fn bench_get_followers_reach_hot_tags(c: &mut Criterion) {
    println!(
        "****************************************************************************************"
    );
    println!(
        "Test the performance of getting tags by followers reach, using index or graph as needed"
    );
    println!(
        "****************************************************************************************"
    );

    run_setup();

    let user_id = "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo";
    let reach_by = format!("{:?}", UserStreamType::Followers);
    let rt: Runtime = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new(
            "bench_get_user_followers_hot_tags",
            format!("user_id: {}, reach: {}", user_id, reach_by),
        ),
        &[user_id],
        |b, &params| {
            b.to_async(&rt).iter(|| async {
                let profile = HotTags::get_stream_tags_by_reach(
                    String::from(params[0]),
                    UserStreamType::Followers,
                )
                .await
                .unwrap();
                criterion::black_box(profile);
            });
        },
    );
}

fn bench_get_friends_reach_hot_tags(c: &mut Criterion) {
    println!(
        "****************************************************************************************"
    );
    println!(
        "Test the performance of getting tags by friends reach, using index or graph as needed"
    );
    println!(
        "****************************************************************************************"
    );

    run_setup();

    let user_id = "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo";
    let reach_by = format!("{:?}", UserStreamType::Friends);
    let rt: Runtime = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new(
            "bench_get_user_friends_hot_tags",
            format!("user_id: {}, reach: {}", user_id, reach_by),
        ),
        &[user_id],
        |b, &params| {
            b.to_async(&rt).iter(|| async {
                let profile = HotTags::get_stream_tags_by_reach(
                    String::from(params[0]),
                    UserStreamType::Friends,
                )
                .await
                .unwrap();
                criterion::black_box(profile);
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
    targets =   bench_get_user_tags,
                bench_get_post_tags,
                bench_get_global_hot_tags,
                bench_get_following_reach_hot_tags,
                bench_get_followers_reach_hot_tags,
                bench_get_friends_reach_hot_tags
}

criterion_main!(benches);
