use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use nexus_common::models::post::{Bookmark, PostCounts, PostDetails, PostRelationships, PostView};
use setup::run_setup;
use std::time::Duration;
use tokio::runtime::Runtime;

mod setup;

fn bench_get_post_by_id(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Test the performance of getting a post by ID, using index or graph as needed");
    println!("******************************************************************************");

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
                let post = PostView::get_by_id(author_id, id, Some(viewer_id), None, None)
                    .await
                    .unwrap();
                std::hint::black_box(post);
            });
        },
    );
}

fn bench_get_post_details_by_id(c: &mut Criterion) {
    println!("******************************************************************************");
    println!(
        "Test the performance of getting a post's details by ID, using index or graph as needed"
    );
    println!("******************************************************************************");

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
                std::hint::black_box(post);
            });
        },
    );
}

fn bench_get_post_details_from_graph(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Test the performance of getting a post's details from the graph database.");
    println!("******************************************************************************");

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
                std::hint::black_box(post);
            });
        },
    );
}

fn bench_get_post_counts_by_id(c: &mut Criterion) {
    println!("******************************************************************************");
    println!(
        "Test the performance of getting a post's counts by ID, using index or graph as needed"
    );
    println!("******************************************************************************");

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
                std::hint::black_box(post);
            });
        },
    );
}

fn bench_get_post_counts_from_graph(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Test the performance of getting a post's counts from the graph database.");
    println!("******************************************************************************");

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
                std::hint::black_box(post);
            });
        },
    );
}

fn bench_get_post_bookmark_by_id(c: &mut Criterion) {
    println!("******************************************************************************");
    println!(
        "Test the performance of getting a post's bookmark by ID, using index or graph as needed"
    );
    println!("******************************************************************************");

    run_setup();

    let author_id = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";
    let post_id = "2ZCW1TGR5BKG0";
    let rt = Runtime::new().unwrap();
    let viewer_id = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";

    c.bench_with_input(
        BenchmarkId::new("get_post_bookmark_by_id", post_id),
        &post_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let post = Bookmark::get_by_id(author_id, id, Some(viewer_id))
                    .await
                    .unwrap();
                std::hint::black_box(post);
            });
        },
    );
}

fn bench_get_post_bookmark_from_graph(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Test the performance of getting a post's bookmark from the graph database.");
    println!("******************************************************************************");

    run_setup();

    let author_id = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";
    let post_id = "2ZCW1TGR5BKG0";
    let viewer_id = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("get_post_bookmark_from_graph", post_id),
        &post_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let post = Bookmark::get_from_graph(author_id, id, viewer_id)
                    .await
                    .unwrap();
                std::hint::black_box(post);
            });
        },
    );
}

fn bench_get_post_relationships_by_id(c: &mut Criterion) {
    println!("******************************************************************************");
    println!(
        "Test the performance of getting a post's relationships by ID, using index or graph as needed"
    );
    println!("******************************************************************************");

    run_setup();

    let author_id = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";
    let post_id = "2ZCW1TGR5BKG0";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("get_post_relationships_by_id", post_id),
        &post_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let post = PostRelationships::get_by_id(author_id, id).await.unwrap();
                std::hint::black_box(post);
            });
        },
    );
}

fn bench_get_post_relationships_from_graph(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Test the performance of getting a post's relationships from the graph database.");
    println!("******************************************************************************");

    run_setup();

    let author_id = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";
    let post_id = "2ZCW1TGR5BKG0";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("get_post_relationships_from_graph", post_id),
        &post_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let post = PostRelationships::get_from_graph(author_id, id)
                    .await
                    .unwrap();
                std::hint::black_box(post);
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
              bench_get_post_bookmark_by_id,
              bench_get_post_bookmark_from_graph,
              bench_get_post_relationships_by_id,
              bench_get_post_relationships_from_graph,
}

criterion_main!(benches);
