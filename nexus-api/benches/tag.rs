use criterion::{criterion_group, criterion_main};
use criterion::{BenchmarkId, Criterion};
use nexus_common::models::tag::global::Taggers;
use nexus_common::models::tag::post::TagPost;
use nexus_common::models::tag::stream::HotTags;
use nexus_common::models::tag::traits::{TagCollection, TaggersCollection};
use nexus_common::models::tag::user::TagUser;
use nexus_common::types::routes::HotTagsInputDTO;
use nexus_common::types::{Pagination, StreamReach, Timeframe};
use setup::run_setup;
use std::time::Duration;
use tokio::runtime::Runtime;

mod setup;

fn bench_get_user_tags(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Test the performance of getting a user tags, using index or graph as needed");
    println!("******************************************************************************");

    run_setup();

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("bench_get_user_tags", user_id),
        &user_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let tag_details_list = TagUser::get_by_id(id, None, None, None, None, None, None)
                    .await
                    .unwrap();
                std::hint::black_box(tag_details_list);
            });
        },
    );
}

fn bench_get_wot_user_tags(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Test the performance of getting a user tags, using index or graph as needed");
    println!("******************************************************************************");

    run_setup();

    let user_id = "c4yotzcb76d31y44jsymtdcowqg7oyqej46jty3yy7ybtzt9x41o";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("bench_get_wot_user_tags", user_id),
        &user_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let tag_details_list = TagUser::get_by_id(
                    id,
                    None,
                    None,
                    None,
                    None,
                    Some("bbkdkhm97pytrb785rdpornkjpcxi331hpq446ckn6rhb4abiguy"),
                    Some(3),
                )
                .await
                .unwrap();
                std::hint::black_box(tag_details_list);
            });
        },
    );
}

fn bench_get_user_tag_taggers(c: &mut Criterion) {
    println!("***************************************************************");
    println!("Test the performance of getting a user tag taggers, using index");
    println!("***************************************************************");

    run_setup();

    let user_id = "5f4e8eoogmkhqeyo5ijdix3ma6rw9byj8m36yrjp78pnxxc379to";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("bench_get_user_tag_taggers", user_id),
        &user_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let taggers =
                    TagUser::get_tagger_by_id(id, None, "pubky", Pagination::default(), None, None)
                        .await
                        .unwrap();
                std::hint::black_box(taggers);
            });
        },
    );
}

fn bench_get_wot_user_tag_taggers(c: &mut Criterion) {
    println!("***************************************************************");
    println!("Test the performance of getting a user tag taggers, using index");
    println!("***************************************************************");

    run_setup();

    let user_id = "c4yotzcb76d31y44jsymtdcowqg7oyqej46jty3yy7ybtzt9x41o";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("bench_get_wot_user_tag_taggers", user_id),
        &user_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let taggers = TagUser::get_tagger_by_id(
                    id,
                    None,
                    "now",
                    Pagination::default(),
                    Some("bbkdkhm97pytrb785rdpornkjpcxi331hpq446ckn6rhb4abiguy"),
                    Some(3),
                )
                .await
                .unwrap();
                std::hint::black_box(taggers);
            });
        },
    );
}

fn bench_get_post_tags(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Test the performance of getting a post tags, using index or graph as needed");
    println!("******************************************************************************");

    run_setup();

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let post_id = "2Z1NJPW2QHGG0";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new(
            "bench_get_post_tags",
            format!("user_id: {user_id}, post_id: {post_id}"),
        ),
        &[user_id, post_id],
        |b, &params| {
            b.to_async(&rt).iter(|| async {
                let tag_details_list =
                    TagPost::get_by_id(params[0], Some(params[1]), None, None, None, None, None)
                        .await
                        .unwrap();
                std::hint::black_box(tag_details_list);
            });
        },
    );
}
fn bench_get_post_tag_taggers(c: &mut Criterion) {
    println!("*****************************************************************");
    println!("Test the performance of getting a post tag taggers, using index");
    println!("*****************************************************************");

    run_setup();

    let user_id = "5f4e8eoogmkhqeyo5ijdix3ma6rw9byj8m36yrjp78pnxxc379to";
    let post_id = "0RDV7ABDZDW0";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new(
            "bench_get_post_tag_taggers",
            format!("user_id: {user_id}, post_id: {post_id}"),
        ),
        &[user_id, post_id],
        |b, &params| {
            b.to_async(&rt).iter(|| async {
                let taggers = TagPost::get_tagger_by_id(
                    params[0],
                    Some(params[1]),
                    "free",
                    Pagination::default(),
                    None,
                    None,
                )
                .await
                .unwrap();
                std::hint::black_box(taggers);
            });
        },
    );
}

fn bench_get_global_hot_tags(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Test the performance of getting a global tags, using index or graph as needed");
    println!("******************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("bench_get_global_hot_tags", |b| {
        b.to_async(&rt).iter(|| async {
            let input = HotTagsInputDTO::new(Timeframe::AllTime, 0, 40, 10, None);
            let stream_tag = HotTags::get_hot_tags(None, None, &input).await.unwrap();
            std::hint::black_box(stream_tag);
        });
    });
}

fn bench_get_global_tag_taggers(c: &mut Criterion) {
    println!("******************************************************************");
    println!("Test the performance of getting global tag taggers");
    println!("******************************************************************");

    run_setup();

    let label = "ha";
    let rt: Runtime = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("bench_get_global_tag_taggers", format!("label: {label}")),
        &[label],
        |b, &params| {
            b.to_async(&rt).iter(|| async {
                let tag_taggers = Taggers::get_global_taggers(
                    String::from(params[0]),
                    None,
                    None,
                    0,
                    20,
                    Timeframe::AllTime,
                )
                .await
                .unwrap();
                std::hint::black_box(tag_taggers);
            });
        },
    );
}

fn bench_get_following_reach_hot_tags(c: &mut Criterion) {
    println!("***************************************************************************");
    println!(
        "Test the performance of getting tags by following reach, using index or graph as needed"
    );
    println!("***************************************************************************");

    run_setup();

    let user_id = "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo";
    let reach_by = format!("{:?}", StreamReach::Following);
    let rt: Runtime = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new(
            "bench_get_following_reach_hot_tags",
            format!("user_id: {user_id}, reach: {reach_by}"),
        ),
        &[user_id],
        |b, &params| {
            b.to_async(&rt).iter(|| async {
                let input = HotTagsInputDTO {
                    timeframe: Timeframe::AllTime,
                    skip: 0,
                    limit: 10,
                    taggers_limit: 20,
                    tagged_type: None,
                };
                let profile = HotTags::get_hot_tags(
                    Some(String::from(params[0])),
                    Some(StreamReach::Following),
                    &input,
                )
                .await
                .unwrap();
                std::hint::black_box(profile);
            });
        },
    );
}

fn bench_get_followers_reach_hot_tags(c: &mut Criterion) {
    println!("***************************************************************************");
    println!(
        "Test the performance of getting tags by followers reach, using index or graph as needed"
    );
    println!("***************************************************************************");

    run_setup();

    let user_id = "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo";
    let reach_by = format!("{:?}", StreamReach::Followers);
    let rt: Runtime = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new(
            "bench_get_followers_reach_hot_tags",
            format!("user_id: {user_id}, reach: {reach_by}"),
        ),
        &[user_id],
        |b, &params| {
            b.to_async(&rt).iter(|| async {
                let input = HotTagsInputDTO {
                    timeframe: Timeframe::AllTime,
                    skip: 0,
                    limit: 10,
                    taggers_limit: 20,
                    tagged_type: None,
                };
                let profile = HotTags::get_hot_tags(
                    Some(String::from(params[0])),
                    Some(StreamReach::Followers),
                    &input,
                )
                .await
                .unwrap();
                std::hint::black_box(profile);
            });
        },
    );
}

fn bench_get_friends_reach_hot_tags(c: &mut Criterion) {
    println!("***************************************************************************");
    println!(
        "Test the performance of getting tags by friends reach, using index or graph as needed"
    );
    println!("***************************************************************************");

    run_setup();

    let user_id = "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo";
    let reach_by = format!("{:?}", StreamReach::Friends);
    let rt: Runtime = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new(
            "bench_get_friends_reach_hot_tags",
            format!("user_id: {user_id}, reach: {reach_by}"),
        ),
        &[user_id],
        |b, &params| {
            b.to_async(&rt).iter(|| async {
                let input = HotTagsInputDTO {
                    timeframe: Timeframe::AllTime,
                    skip: 0,
                    limit: 10,
                    taggers_limit: 20,
                    tagged_type: None,
                };
                let profile = HotTags::get_hot_tags(
                    Some(String::from(params[0])),
                    Some(StreamReach::Friends),
                    &input,
                )
                .await
                .unwrap();
                std::hint::black_box(profile);
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
    targets =   bench_get_user_tags,
                bench_get_wot_user_tags,
                bench_get_user_tag_taggers,
                bench_get_wot_user_tag_taggers,
                bench_get_post_tags,
                bench_get_post_tag_taggers,
                bench_get_global_hot_tags,
                bench_get_global_tag_taggers,
                bench_get_following_reach_hot_tags,
                bench_get_followers_reach_hot_tags,
                bench_get_friends_reach_hot_tags
}

criterion_main!(benches);
