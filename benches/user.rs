use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use pubky_nexus::models::user::{Relationship, UserCounts, UserDetails, UserView};
use pubky_nexus::setup;
use pubky_nexus::Config;
use std::env;
use std::time::Duration;
use tokio::runtime::Runtime;

use std::sync::Once;

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

fn bench_get_full_by_id(c: &mut Criterion) {
    println!("***************************************");
    println!("Test the performance of getting a user view by ID, using index or graph as needed");
    println!("***************************************");

    run_setup();

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let viewer_id = "5g3fwnue819wfdjwiwm8qr35ww6uxxgbzrigrtdgmbi19ksioeoy"; // Provide the viewer_id
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("get_full_profile_by_id", user_id),
        &user_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let profile = UserView::get_by_id(id, Some(viewer_id)).await.unwrap();
                criterion::black_box(profile);
            });
        },
    );
}

fn bench_get_relationship_from_graph(c: &mut Criterion) {
    println!("***************************************");
    println!("Test the performance of getting a following/follower relationship from the graph database.");
    println!("***************************************");

    run_setup();

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let viewer_id = "5g3fwnue819wfdjwiwm8qr35ww6uxxgbzrigrtdgmbi19ksioeoy";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("get_relationship_from_graph", user_id),
        &user_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let relationship = Relationship::get_from_graph(id, viewer_id).await.unwrap();
                criterion::black_box(relationship);
            });
        },
    );
}

fn bench_get_relationship_by_id(c: &mut Criterion) {
    println!("***************************************");
    println!("Test the performance of getting a following/follower relationship by ID, using index or graph as needed.");
    println!("***************************************");

    run_setup();

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let viewer_id = "5g3fwnue819wfdjwiwm8qr35ww6uxxgbzrigrtdgmbi19ksioeoy";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("get_relationship_by_id", user_id),
        &user_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let relationship = Relationship::get_by_id(id, Some(viewer_id)).await.unwrap();
                criterion::black_box(relationship);
            });
        },
    );
}

fn bench_get_counts_from_graph(c: &mut Criterion) {
    println!("***************************************");
    println!("Test the performance of getting profile counts from the graph database.");
    println!("***************************************");

    run_setup();

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("get_counts_from_graph", user_id),
        &user_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let counts = UserCounts::get_from_graph(id).await.unwrap();
                criterion::black_box(counts);
            });
        },
    );
}

fn bench_get_counts_by_id(c: &mut Criterion) {
    println!("***************************************");
    println!(
        "Test the performance of getting profile counts by ID, using index or graph as needed."
    );
    println!("***************************************");

    run_setup();

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("get_counts_by_id", user_id),
        &user_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let counts = UserCounts::get_by_id(id).await.unwrap();
                criterion::black_box(counts);
            });
        },
    );
}

fn bench_get_details_from_graph(c: &mut Criterion) {
    println!("***************************************");
    println!("Test the performance of getting profile details from the graph database.");
    println!("***************************************");

    run_setup();

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("get_details_from_graph", user_id),
        &user_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let details = UserDetails::get_from_graph(id).await.unwrap();
                criterion::black_box(details);
            });
        },
    );
}

fn bench_get_details_by_id(c: &mut Criterion) {
    println!("***************************************");
    println!(
        "Test the performance of getting profile details by ID, checking both index and graph."
    );
    println!("***************************************");

    run_setup();

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("get_details_by_id", user_id),
        &user_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let details = UserDetails::get_by_id(id).await.unwrap();
                criterion::black_box(details);
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
    targets = bench_get_full_by_id,
              bench_get_relationship_from_graph,
              bench_get_relationship_by_id,
              bench_get_counts_from_graph,
              bench_get_counts_by_id,
              bench_get_details_from_graph,
              bench_get_details_by_id
}

criterion_main!(benches);
