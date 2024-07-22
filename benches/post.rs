use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use pubky_nexus::models::post::Post;
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

    let author_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let post_id = "2Z1NJPW2QHGG0";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("get_post_by_id", post_id),
        &post_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let post = Post::get_by_id(author_id, id).await.unwrap();
                criterion::black_box(post);
            });
        },
    );
}

fn bench_get_post_from_graph(c: &mut Criterion) {
    println!("***************************************");
    println!("Test the performance of getting a post from the graph database.");
    println!("***************************************");

    run_setup();

    let author_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let post_id = "2Z1NJPW2QHGG0";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("get_post_from_graph", post_id),
        &post_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let post = Post::get_from_graph(author_id, id).await.unwrap();
                criterion::black_box(post);
            });
        },
    );
}

fn configure_criterion() -> Criterion {
    Criterion::default()
        .measurement_time(Duration::new(5, 0))
        .sample_size(200)
        .warm_up_time(Duration::new(1, 0))
}

criterion_group! {
    name = benches;
    config = configure_criterion();
    targets = bench_get_post_by_id,
              bench_get_post_from_graph
}

criterion_main!(benches);
