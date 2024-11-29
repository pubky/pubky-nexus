use chrono::Utc;
use criterion::{criterion_group, criterion_main};
use criterion::{BenchmarkId, Criterion};
use pubky_nexus::models::tag::global::TagGlobal;
use pubky_nexus::models::tag::post::TagPost;
use pubky_nexus::models::tag::stream::{HotTags, HotTagsInput, TagStreamReach, Timeframe};
use pubky_nexus::models::tag::traits::{TagCollection, TaggersCollection};
use pubky_nexus::models::tag::user::TagUser;
use pubky_nexus::types::Pagination;
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
        BenchmarkId::new("bench_get_wot_user_tags", user_id),
        &user_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let tag_details_list = TagUser::get_by_id(id, None, None, None, None, None)
                    .await
                    .unwrap();
                criterion::black_box(tag_details_list);
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
        BenchmarkId::new("bench_get_user_tags", user_id),
        &user_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let tag_details_list = TagUser::get_by_id(
                    id,
                    None,
                    None,
                    None,
                    Some("bbkdkhm97pytrb785rdpornkjpcxi331hpq446ckn6rhb4abiguy"),
                    Some(3),
                )
                .await
                .unwrap();
                criterion::black_box(tag_details_list);
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
                criterion::black_box(taggers);
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
        BenchmarkId::new("bench_get_user_tag_taggers", user_id),
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
                criterion::black_box(taggers);
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
            format!("user_id: {}, post_id: {}", user_id, post_id),
        ),
        &[user_id, post_id],
        |b, &params| {
            b.to_async(&rt).iter(|| async {
                let tag_details_list =
                    TagPost::get_by_id(params[0], Some(params[1]), None, None, None, None)
                        .await
                        .unwrap();
                criterion::black_box(tag_details_list);
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
            format!("user_id: {}, post_id: {}", user_id, post_id),
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
                criterion::black_box(taggers);
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
            let input = HotTagsInput {
                timeframe: Timeframe::AllTime,
                skip: 0,
                limit: 40,
                taggers_limit: 10,
                tagged_type: None,
            };
            let stream_tag = HotTags::get_hot_tags(None, None, &input).await.unwrap();
            criterion::black_box(stream_tag);
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
        BenchmarkId::new("bench_get_global_tag_taggers", format!("label: {}", label)),
        &[label],
        |b, &params| {
            b.to_async(&rt).iter(|| async {
                let tag_taggers =
                    TagGlobal::get_tag_taggers(String::from(params[0]), None, None, 0, 20)
                        .await
                        .unwrap();
                criterion::black_box(tag_taggers);
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
    let reach_by = format!("{:?}", TagStreamReach::Following);
    let rt: Runtime = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new(
            "bench_get_user_following_hot_tags",
            format!("user_id: {}, reach: {}", user_id, reach_by),
        ),
        &[user_id],
        |b, &params| {
            b.to_async(&rt).iter(|| async {
                let input = HotTagsInput {
                    timeframe: Timeframe::AllTime,
                    skip: 0,
                    limit: 10,
                    taggers_limit: 20,
                    tagged_type: None,
                };
                let profile = HotTags::get_hot_tags(
                    Some(String::from(params[0])),
                    Some(TagStreamReach::Following),
                    &input,
                )
                .await
                .unwrap();
                criterion::black_box(profile);
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
    let reach_by = format!("{:?}", TagStreamReach::Followers);
    let rt: Runtime = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new(
            "bench_get_user_followers_hot_tags",
            format!("user_id: {}, reach: {}", user_id, reach_by),
        ),
        &[user_id],
        |b, &params| {
            b.to_async(&rt).iter(|| async {
                let input = HotTagsInput {
                    timeframe: Timeframe::AllTime,
                    skip: 0,
                    limit: 10,
                    taggers_limit: 20,
                    tagged_type: None,
                };
                let profile = HotTags::get_hot_tags(
                    Some(String::from(params[0])),
                    Some(TagStreamReach::Followers),
                    &input,
                )
                .await
                .unwrap();
                criterion::black_box(profile);
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
    let reach_by = format!("{:?}", TagStreamReach::Friends);
    let rt: Runtime = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new(
            "bench_get_user_friends_hot_tags",
            format!("user_id: {}, reach: {}", user_id, reach_by),
        ),
        &[user_id],
        |b, &params| {
            b.to_async(&rt).iter(|| async {
                let input = HotTagsInput {
                    timeframe: Timeframe::AllTime,
                    skip: 0,
                    limit: 10,
                    taggers_limit: 20,
                    tagged_type: None,
                };
                let profile = HotTags::get_hot_tags(
                    Some(String::from(params[0])),
                    Some(TagStreamReach::Friends),
                    &input,
                )
                .await
                .unwrap();
                criterion::black_box(profile);
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
