use crate::run_setup;
use crate::streams_benches::LIMIT_20;
use criterion::Criterion;
use nexus_common::db::kv::SortOrder;
use nexus_common::models::resource::stream::{
    ResourceSorting, ResourceStream, ResourceStreamSource,
};
use tokio::runtime::Runtime;

// Fixture data (docker/test-graph/mocks/resources.cypher) uses these; a
// synthetic graph should make them its most popular app and label so the same
// targets stay meaningful at scale.
const APP: &str = "mapky";
const TAG: &str = "bitcoin";

/// RESOURCE STREAM BENCHMARKS (served from the graph)
fn bench_resource_keys(
    c: &mut Criterion,
    name: &str,
    app: Option<&'static str>,
    tag: Option<&'static str>,
    sorting: ResourceSorting,
) {
    println!("******************************************************************************");
    println!("Benchmarking the resource stream '{name}'.");
    println!("******************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function(name, |b| {
        b.to_async(&rt).iter(|| async {
            let source = match app {
                Some(app) => ResourceStreamSource::App {
                    app: app.to_string(),
                },
                None => ResourceStreamSource::All,
            };
            let tags = tag.map(|t| vec![t.to_string()]);

            let keys = ResourceStream::get_resource_keys(
                &source,
                LIMIT_20,
                SortOrder::Descending,
                &sorting,
                tags.as_deref(),
            )
            .await
            .unwrap();
            std::hint::black_box(keys);
        });
    });
}

pub fn bench_stream_resources_all_timeline(c: &mut Criterion) {
    bench_resource_keys(
        c,
        "stream_resources_all_timeline",
        None,
        None,
        ResourceSorting::Timeline,
    );
}

pub fn bench_stream_resources_all_taggers_count(c: &mut Criterion) {
    bench_resource_keys(
        c,
        "stream_resources_all_taggers_count",
        None,
        None,
        ResourceSorting::TaggersCount,
    );
}

pub fn bench_stream_resources_app_timeline(c: &mut Criterion) {
    bench_resource_keys(
        c,
        "stream_resources_app_timeline",
        Some(APP),
        None,
        ResourceSorting::Timeline,
    );
}

pub fn bench_stream_resources_app_taggers_count(c: &mut Criterion) {
    bench_resource_keys(
        c,
        "stream_resources_app_taggers_count",
        Some(APP),
        None,
        ResourceSorting::TaggersCount,
    );
}

pub fn bench_stream_resources_tag_timeline(c: &mut Criterion) {
    bench_resource_keys(
        c,
        "stream_resources_tag_timeline",
        None,
        Some(TAG),
        ResourceSorting::Timeline,
    );
}

pub fn bench_stream_resources_tag_taggers_count(c: &mut Criterion) {
    bench_resource_keys(
        c,
        "stream_resources_tag_taggers_count",
        None,
        Some(TAG),
        ResourceSorting::TaggersCount,
    );
}

pub fn bench_stream_resources_app_tag_timeline(c: &mut Criterion) {
    bench_resource_keys(
        c,
        "stream_resources_app_tag_timeline",
        Some(APP),
        Some(TAG),
        ResourceSorting::Timeline,
    );
}

pub fn bench_stream_resources_app_tag_taggers_count(c: &mut Criterion) {
    bench_resource_keys(
        c,
        "stream_resources_app_tag_taggers_count",
        Some(APP),
        Some(TAG),
        ResourceSorting::TaggersCount,
    );
}
