use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use pk_social_service::config::Config;
use pk_social_service::models::profile::Profile;
use pk_social_service::setup;
use tokio::runtime::Runtime;

pub fn run_setup() {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        let config = Config::from_env();
        setup::setup(&config).await;
    });
}

fn bench_profile(c: &mut Criterion) {
    run_setup();

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("get_profile_by_id", user_id),
        &user_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let profile = Profile::get_by_id(id).await.unwrap();
                criterion::black_box(profile);
            });
        },
    );
}

criterion_group!(benches, bench_profile);
criterion_main!(benches);
