use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use nexus_common::models::follow::{Followers, Following, UserFollows};
use setup::run_setup;
use std::time::Duration;
use tokio::runtime::Runtime;

mod setup;

fn bench_get_followers_by_id(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Test the performance of getting followers by ID, using index or graph as needed.");
    println!("******************************************************************************");

    run_setup();

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("get_followers_by_id", user_id),
        &user_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let followers = Followers::get_by_id(id, None, None).await.unwrap();
                std::hint::black_box(followers);
            });
        },
    );
}

fn bench_get_followers_from_graph(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Test the performance of getting followers from the graph database.");
    println!("******************************************************************************");

    run_setup();

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("get_followers_from_graph", user_id),
        &user_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let followers = Followers::get_from_graph(id, None, None).await.unwrap();
                std::hint::black_box(followers);
            });
        },
    );
}

fn bench_get_following_by_id(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Test the performance of getting following by ID, using index or graph as needed.");
    println!("******************************************************************************");

    run_setup();

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("get_following_by_id", user_id),
        &user_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let following = Following::get_by_id(id, None, None).await.unwrap();
                std::hint::black_box(following);
            });
        },
    );
}

fn bench_get_following_from_graph(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Test the performance of getting following from the graph database.");
    println!("******************************************************************************");

    run_setup();

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("get_following_from_graph", user_id),
        &user_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let following = Following::get_from_graph(id, None, None).await.unwrap();
                std::hint::black_box(following);
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
    targets = bench_get_followers_by_id,
              bench_get_followers_from_graph,
              bench_get_following_by_id,
              bench_get_following_from_graph,
}

criterion_main!(benches);
