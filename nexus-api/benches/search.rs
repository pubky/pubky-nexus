use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use nexus_common::{
    models::{post::search::PostsByTagSearch, tag::search::TagSearch, user::UserSearch},
    types::Pagination,
};
use setup::run_setup;
use std::time::Duration;
use tokio::runtime::Runtime;

mod setup;

fn bench_user_search(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Benchmarking the user search.");
    println!("******************************************************************************");

    run_setup();

    let username = "a"; // Will match the anonymous users
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("user_search", username),
        &username,
        |b, &username| {
            b.to_async(&rt).iter(|| async {
                let result = UserSearch::get_by_name(username, None, Some(40))
                    .await
                    .unwrap();
                std::hint::black_box(result);
            });
        },
    );
}

fn bench_tag_search(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Benchmarking the tag search.");
    println!("******************************************************************************");

    run_setup();

    let tag_prefix = "he"; // Matches 3 tags
    let rt = Runtime::new().unwrap();

    let pagination = Pagination::default();
    c.bench_with_input(
        BenchmarkId::new("tag_search", tag_prefix),
        &tag_prefix,
        |b, &prefix| {
            b.to_async(&rt).iter(|| async {
                let result = TagSearch::get_by_label(prefix, &pagination).await.unwrap();
                std::hint::black_box(result);
            });
        },
    );
}

fn bench_post_tag_search_by_timeline(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Benchmarking post tag search by timeline");
    println!("******************************************************************************");

    run_setup();

    let label = "free";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("post_tag_search_by_timeline", label),
        &label,
        |b, &label| {
            b.to_async(&rt).iter(|| async {
                let pagination = Pagination {
                    skip: Some(0),
                    limit: Some(20),
                    start: None,
                    end: None,
                };
                let result = PostsByTagSearch::get_by_label(label, None, pagination)
                    .await
                    .unwrap();
                std::hint::black_box(result);
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
    targets =   bench_user_search,
                bench_tag_search,
                bench_post_tag_search_by_timeline
}

criterion_main!(benches);
