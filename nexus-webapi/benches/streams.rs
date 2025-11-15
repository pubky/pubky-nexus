use criterion::{criterion_group, criterion_main, Criterion};
use std::time::Duration;
use streams_benches::{author, bookmarks, kind, post_keys, reach, sorting, tag, user};

mod setup;
mod streams_benches;
pub use setup::run_setup;

fn configure_criterion() -> Criterion {
    Criterion::default()
        .measurement_time(Duration::new(5, 0))
        .sample_size(100)
        .warm_up_time(Duration::new(1, 0))
}

criterion_group! {
    name = streams;
    config = configure_criterion();
    targets = bookmarks::bench_stream_bookmarks_timeline,
              bookmarks::bench_stream_bookmarks_total_engagement,
              author::bench_stream_author_timeline,
              author::bench_stream_author_total_engagement,
              author::bench_stream_author_replies_timeline,
              reach::bench_stream_followers_timeline,
              reach::bench_stream_following_timeline,
              reach::bench_stream_friends_timeline,
              reach::bench_stream_followers_total_engagement,
              reach::bench_stream_following_total_engagement,
              reach::bench_stream_friends_total_engagement,
              sorting::bench_stream_all_timeline,
              sorting::bench_stream_all_total_engagement,
              post_keys::bench_stream_post_keys_all_timeline,
              tag::bench_stream_tag_timeline,
              tag::bench_stream_tag_total_engagement,
              user::bench_stream_users_by_username_search,
              user::bench_stream_influencers,
              user::bench_stream_following,
              user::bench_stream_most_followed,
              user::bench_stream_post_replies,
              user::bench_stream_user_ids_most_followed,
              kind::bench_stream_post_kind_short,
              kind::bench_stream_post_kind_long,
              kind::bench_stream_post_kind_image,
              kind::bench_stream_post_kind_video,
              kind::bench_stream_post_kind_link,
              kind::bench_stream_post_kind_file
}

criterion_main!(streams);
