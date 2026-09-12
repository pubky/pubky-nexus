//! Query performance audit: dumper.
//!
//! Prints a `PROFILE <cypher>` block for each audited graph query, using the
//! REAL query functions from `nexus_common` (via `Query::to_cypher_populated`),
//! so the profiled Cypher can never drift from production. It does not touch the
//! database; `docker/bench-graph/run.sh` pipes these blocks into cypher-shell.
//!
//! The literal ids below match the entities created by `docker/bench-graph/seed.sh`.
//! To audit another query, add one `emit(...)` line; if the signature changes,
//! this stops compiling, which is the point.
//!
//! `tier` reflects blast radius: 0 = hot reads (API + cache-miss + reindex),
//! 1 = write-path / notification-fan-out reads, 2 = rare/admin.

use nexus_common::db::graph::Query;
use nexus_common::db::kv::SortOrder;
use nexus_common::db::queries::{del, get, put};
use nexus_common::models::post::StreamSource;
use nexus_common::models::resource::stream::ResourceSorting;
use nexus_common::types::routes::HotTagsInputDTO;
use nexus_common::types::{Pagination, StreamReach, StreamSorting, Timeframe, WotDepth};

const SUBJECT: &str = "audit_subject";
const VIEWER: &str = "audit_viewer";
const HOT_POST: &str = "audit_post_1";
const RESOURCE: &str = "audit_resource";
const LABEL: &str = "audit_label";

/// Emit one profiled read query, delimited for the runner to split on.
fn emit(tier: u8, label: &str, q: Query) {
    emit_marker("@@QUERY", tier, label, q);
}

/// Emit a write query; the runner wraps these in `:begin`/`:rollback` so PROFILE
/// measures the write without mutating the graph.
fn emit_write(tier: u8, label: &str, q: Query) {
    emit_marker("@@WRITE", tier, label, q);
}

fn emit_marker(marker: &str, tier: u8, label: &str, q: Query) {
    let cypher = q.to_cypher_populated();
    let body = cypher.trim().trim_end_matches(';');
    println!("{marker} {tier} {label}");
    println!("PROFILE\n{body}\n;");
    println!("@@END");
}

fn hot_tags_dto() -> HotTagsInputDTO {
    HotTagsInputDTO {
        timeframe: Timeframe::AllTime,
        skip: 0,
        limit: 20,
        taggers_limit: 10,
        tagged_type: None,
    }
}

fn pg() -> Pagination {
    Pagination {
        skip: Some(0),
        limit: Some(30),
        start: None,
        end: None,
    }
}

fn main() {
    // ---- Tier 0: hot reads ----
    emit(0, "user_counts", get::user_counts(SUBJECT));
    emit(0, "post_counts", get::post_counts(SUBJECT, HOT_POST));
    emit(0, "get_post_by_id", get::get_post_by_id(SUBJECT, HOT_POST));
    emit(
        0,
        "post_relationships",
        get::post_relationships(SUBJECT, HOT_POST),
    );
    emit(
        0,
        "get_users_details_by_ids",
        get::get_users_details_by_ids(&[SUBJECT, VIEWER]),
    );
    emit(
        0,
        "get_user_followers",
        get::get_user_followers(SUBJECT, Some(0), Some(30)),
    );
    emit(
        0,
        "get_user_following",
        get::get_user_following(SUBJECT, Some(0), Some(30)),
    );
    emit(0, "get_post_tags", get::get_post_tags(SUBJECT, HOT_POST));
    emit(0, "post_tags", get::post_tags(SUBJECT, HOT_POST));
    emit(0, "user_tags", get::user_tags(SUBJECT));
    emit(0, "get_tags", get::get_tags());
    emit(
        0,
        "get_tags_by_label_prefix",
        get::get_tags_by_label_prefix("rl_"),
    );
    emit(0, "global_tags_by_post", get::global_tags_by_post());
    emit(
        0,
        "global_tags_by_post_engagement",
        get::global_tags_by_post_engagement(),
    );
    emit(0, "recommend_users", get::recommend_users(SUBJECT, 10));
    emit(
        0,
        "get_global_influencers",
        get::get_global_influencers(0, 20, &Timeframe::AllTime),
    );
    emit(
        0,
        "get_influencers_by_reach",
        get::get_influencers_by_reach(SUBJECT, StreamReach::Followers, 0, 20, &Timeframe::AllTime),
    );
    emit(
        0,
        "get_global_hot_tags",
        get::get_global_hot_tags(&hot_tags_dto()),
    );
    emit(
        0,
        "get_hot_tags_by_reach",
        get::get_hot_tags_by_reach(SUBJECT, StreamReach::Followers, &hot_tags_dto()),
    );
    emit(
        0,
        "get_tag_taggers_by_reach",
        get::get_tag_taggers_by_reach(LABEL, SUBJECT, StreamReach::Followers, 0, 30),
    );
    emit(
        0,
        "get_viewer_trusted_network_tags",
        get::get_viewer_trusted_network_tags(SUBJECT, VIEWER, WotDepth::default()),
    );
    emit(
        0,
        "get_viewer_trusted_network_post_tags",
        get::get_viewer_trusted_network_post_tags(
            SUBJECT,
            HOT_POST,
            VIEWER,
            WotDepth::default(),
            0,
            20,
            10,
        ),
    );
    emit(0, "get_resource_by_id", get::get_resource_by_id(RESOURCE));
    emit(0, "resource_tags", get::resource_tags(RESOURCE));

    // Streams (post_stream is a builder: profile representative sources/sortings).
    emit(
        0,
        "post_stream_all_timeline",
        get::post_stream(
            StreamSource::All,
            StreamSorting::Timeline,
            SortOrder::Descending,
            &None,
            pg(),
            None,
        )
        .unwrap(),
    );
    emit(
        0,
        "post_stream_author_engagement",
        get::post_stream(
            StreamSource::Author {
                author_id: SUBJECT.to_string(),
            },
            StreamSorting::TotalEngagement,
            SortOrder::Descending,
            &None,
            pg(),
            None,
        )
        .unwrap(),
    );
    emit(
        0,
        "post_stream_wot_timeline",
        get::post_stream(
            StreamSource::Wot {
                observer_id: VIEWER.to_string(),
                depth: WotDepth::default(),
            },
            StreamSorting::Timeline,
            SortOrder::Descending,
            &None,
            pg(),
            None,
        )
        .unwrap(),
    );
    emit(
        0,
        "post_stream_following_timeline",
        get::post_stream(
            StreamSource::Following {
                observer_id: "audit_follower_1".to_string(),
            },
            StreamSorting::Timeline,
            SortOrder::Descending,
            &None,
            pg(),
            None,
        )
        .unwrap(),
    );
    emit(
        0,
        "resource_stream",
        get::resource_stream(
            None,
            None,
            &ResourceSorting::TaggersCount,
            &SortOrder::Descending,
            0,
            30,
        ),
    );

    // ---- Tier 1: write-path / notification-fan-out reads ----
    emit(
        1,
        "get_post_bookmarks",
        get::get_post_bookmarks(SUBJECT, HOT_POST),
    );
    emit(
        1,
        "get_post_reposts",
        get::get_post_reposts(SUBJECT, HOT_POST),
    );
    emit(
        1,
        "get_post_replies",
        get::get_post_replies(SUBJECT, HOT_POST),
    );
    emit(1, "user_bookmarks", get::user_bookmarks(SUBJECT));
    emit(
        1,
        "post_bookmark",
        get::post_bookmark(SUBJECT, HOT_POST, VIEWER),
    );
    emit(
        1,
        "get_bookmark_target",
        get::get_bookmark_target(SUBJECT, "audit_bm_1"),
    );
    emit(
        1,
        "get_tag_target",
        get::get_tag_target(SUBJECT, "audit_giventag_p1", None),
    );
    emit(
        1,
        "get_tag_by_tagger_and_id",
        get::get_tag_by_tagger_and_id("audit_hp_tagger_1", "audit_hpt_1"),
    );
    emit(
        1,
        "user_is_safe_to_delete",
        get::user_is_safe_to_delete(SUBJECT),
    );
    emit(
        1,
        "post_is_safe_to_delete",
        get::post_is_safe_to_delete(SUBJECT, HOT_POST),
    );

    // ---- Tier 2: rare / admin ----
    emit(
        2,
        "get_homeserver_by_id",
        get::get_homeserver_by_id("audit_hs"),
    );
    emit(2, "get_all_homeservers", get::get_all_homeservers());
    let file_pair: &[&str] = &[SUBJECT, "audit_file_1"];
    let file_pairs: &[&[&str]] = &[file_pair];
    emit(2, "get_files_by_ids", get::get_files_by_ids(file_pairs));

    // ---- Writes: profiled under :begin/:rollback (no mutation). Deletes are the
    // interesting ones (DETACH DELETE cost scales with entity degree); creates are
    // point inserts. Struct-arg creates (create_user/post/file, *_tag, bookmark,
    // mention) are omitted here, they are O(1) MERGEs by id. ----
    emit_write(1, "delete_user", del::delete_user(SUBJECT));
    emit_write(1, "delete_post", del::delete_post(SUBJECT, HOT_POST));
    emit_write(
        1,
        "delete_follow",
        del::delete_follow(SUBJECT, "audit_followee_1"),
    );
    emit_write(
        1,
        "delete_bookmark",
        del::delete_bookmark(SUBJECT, "audit_bm_1"),
    );
    emit_write(
        1,
        "delete_tag",
        del::delete_tag(SUBJECT, "audit_giventag_p1", None),
    );
    emit_write(1, "delete_file", del::delete_file(SUBJECT, "audit_file_1"));
    emit_write(
        1,
        "create_follow",
        put::create_follow(VIEWER, SUBJECT, 1_735_689_600_000),
    );
    emit_write(
        2,
        "create_homeserver",
        put::create_homeserver("audit_hs_probe"),
    );
}
