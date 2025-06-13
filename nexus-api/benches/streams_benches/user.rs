use crate::run_setup;
use criterion::Criterion;
use nexus_common::{
    models::user::{UserStream, UserStreamInput, UserStreamSource},
    types::StreamReach,
};
use tokio::runtime::Runtime;

/// USER STREAMS BENCHMARKS
pub fn bench_stream_following(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Benchmarking the user streams for following users.");
    println!("******************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";

    c.bench_function("stream_following", |b| {
        b.to_async(&rt).iter(|| async {
            let user_stream = UserStream::get_by_id(
                UserStreamInput {
                    user_id: Some(String::from(user_id)),
                    skip: None,
                    limit: Some(20),
                    source: UserStreamSource::Influencers,
                    reach: Some(StreamReach::Following),
                    timeframe: None,
                    preview: None,
                    author_id: None,
                    post_id: None,
                },
                None,
                None,
            )
            .await
            .unwrap();
            std::hint::black_box(user_stream);
        });
    });
}

pub fn bench_stream_most_followed(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Benchmarking the user streams for most followed users.");
    println!("******************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("stream_most_followed", |b| {
        b.to_async(&rt).iter(|| async {
            let user_stream = UserStream::get_by_id(
                UserStreamInput {
                    user_id: None,
                    skip: None,
                    limit: Some(20),
                    source: UserStreamSource::MostFollowed,
                    reach: None,
                    timeframe: None,
                    preview: None,
                    author_id: None,
                    post_id: None,
                },
                None,
                None,
            )
            .await
            .unwrap();
            std::hint::black_box(user_stream);
        });
    });
}

pub fn bench_stream_users_by_username_search(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Benchmarking the user streams by username search.");
    println!("******************************************************************************");

    run_setup();

    let username = "An"; // Match all anonymous profiles
    let rt = Runtime::new().unwrap();

    c.bench_function("stream_users_by_username_search", |b| {
        b.to_async(&rt).iter(|| async {
            let user_stream = UserStream::get_from_username_search(
                username,
                None,
                None,
                Some(40), // Limit to 40 results
            )
            .await
            .unwrap();
            std::hint::black_box(user_stream);
        });
    });
}

pub fn bench_stream_influencers(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Benchmarking the user streams for influencer users.");
    println!("******************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("stream_influencers", |b| {
        b.to_async(&rt).iter(|| async {
            let user_stream = UserStream::get_by_id(
                UserStreamInput {
                    user_id: None,
                    skip: None,
                    limit: Some(20),
                    source: UserStreamSource::Influencers,
                    reach: Some(StreamReach::Wot(3)),
                    author_id: None,
                    post_id: None,
                    timeframe: None,
                    preview: None,
                },
                None,
                None,
            )
            .await
            .unwrap();
            std::hint::black_box(user_stream);
        });
    });
}

pub fn bench_stream_post_replies(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Benchmarking the user streams for a post's replying users.");
    println!("******************************************************************************");

    run_setup();

    let author_id = "emq37ky6fbnaun7q1ris6rx3mqmw3a33so1txfesg9jj3ak9ryoy";
    let post_id = "1A1P4D8C9K0F";
    let rt = Runtime::new().unwrap();

    c.bench_function("stream_post_replies", |b| {
        b.to_async(&rt).iter(|| async {
            let user_stream = UserStream::get_by_id(
                UserStreamInput {
                    user_id: None,
                    skip: None,
                    limit: Some(20),
                    source: UserStreamSource::PostReplies,
                    author_id: Some(author_id.to_string()),
                    post_id: Some(post_id.to_string()),
                    reach: None,
                    timeframe: None,
                    preview: None,
                },
                None,
                None,
            )
            .await
            .unwrap();
            std::hint::black_box(user_stream);
        });
    });
}
