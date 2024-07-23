use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use pubky_nexus::models::post::PostCounts;
use pubky_nexus::models::post::PostDetails;
use pubky_nexus::models::post::PostView;
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

fn bench_get_post_by_id(c: &mut Criterion) {
    println!("***************************************");
    println!("Test the performance of getting a post by ID, using index or graph as needed");
    println!("***************************************");

    run_setup();

    let author_id = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";
    let post_id = "2ZCW1TGR5BKG0";
    let viewer_id = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("get_post_by_id", post_id),
        &post_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let post = PostView::get_by_id(author_id, id, Some(viewer_id))
                    .await
                    .unwrap();
                criterion::black_box(post);
            });
        },
    );
}

fn bench_get_post_details_by_id(c: &mut Criterion) {
    println!("***************************************");
    println!(
        "Test the performance of getting a post's details by ID, using index or graph as needed"
    );
    println!("***************************************");

    run_setup();

    let author_id = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";
    let post_id = "2ZCW1TGR5BKG0";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("get_post_details_by_id", post_id),
        &post_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let post = PostDetails::get_by_id(author_id, id).await.unwrap();
                criterion::black_box(post);
            });
        },
    );
}

fn bench_get_post_details_from_graph(c: &mut Criterion) {
    println!("***************************************");
    println!("Test the performance of getting a post's details from the graph database.");
    println!("***************************************");

    run_setup();

    let author_id = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";
    let post_id = "2ZCW1TGR5BKG0";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("get_post_details_from_graph", post_id),
        &post_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let post = PostDetails::get_from_graph(author_id, id).await.unwrap();
                criterion::black_box(post);
            });
        },
    );
}

fn bench_get_post_counts_by_id(c: &mut Criterion) {
    println!("***************************************");
    println!(
        "Test the performance of getting a post's counts by ID, using index or graph as needed"
    );
    println!("***************************************");

    run_setup();

    let author_id = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";
    let post_id = "2ZCW1TGR5BKG0";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("get_post_counts_by_id", post_id),
        &post_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let post = PostCounts::get_by_id(author_id, id).await.unwrap();
                criterion::black_box(post);
            });
        },
    );
}

fn bench_get_post_counts_from_graph(c: &mut Criterion) {
    println!("***************************************");
    println!("Test the performance of getting a post's counts from the graph database.");
    println!("***************************************");

    run_setup();

    let author_id = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";
    let post_id = "2ZCW1TGR5BKG0";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("get_post_counts_from_graph", post_id),
        &post_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let post = PostCounts::get_from_graph(author_id, id).await.unwrap();
                criterion::black_box(post);
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
    targets = bench_get_post_by_id,
              bench_get_post_details_by_id,
              bench_get_post_details_from_graph,
              bench_get_post_counts_by_id,
              bench_get_post_counts_from_graph,
}

criterion_main!(benches);
