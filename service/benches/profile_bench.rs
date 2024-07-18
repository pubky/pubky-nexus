use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use pk_social_service::models::profile::Profile;
use tokio::runtime::Runtime;

mod setup;

fn bench_profile(c: &mut Criterion) {
    setup::setup();

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
