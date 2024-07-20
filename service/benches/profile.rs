use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use pk_social_service::config::Config;
use pk_social_service::models::profile::Profile;
use pk_social_service::setup;
use std::time::Duration;
use tokio::runtime::Runtime;

pub fn run_setup() {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        let config = Config::from_env();
        setup::setup(&config).await;
    });
}

/// Get a profile by Id will get from Index or Graph as available and return as well the Viewer relationship
fn bench_get_by_id(c: &mut Criterion) {
    run_setup();

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let viewer_id = "5g3fwnue819wfdjwiwm8qr35ww6uxxgbzrigrtdgmbi19ksioeoy"; // Provide the viewer_id
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("get_profile_by_id", user_id),
        &user_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let profile = Profile::get_by_id(id, Some(viewer_id)).await.unwrap();
                criterion::black_box(profile);
            });
        },
    );
}

/// Get a profile from Graph. Does not retrieve the Relationship model.
fn bench_get_from_graph(c: &mut Criterion) {
    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("get_from_graph", user_id),
        &user_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let profile = Profile::get_from_graph(id).await.unwrap();
                criterion::black_box(profile);
            });
        },
    );
}

/// Get a profile from Index. Does not retrieve the Relationship model.
fn bench_get_from_index(c: &mut Criterion) {
    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("get_from_index", user_id),
        &user_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let profile = Profile::get_from_index(id).await.unwrap();
                criterion::black_box(profile);
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
    targets = bench_get_by_id, bench_get_from_graph, bench_get_from_index
}

criterion_main!(benches);
