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

use nexus_common::db::graph::Query;
use nexus_common::db::queries::get;
use nexus_common::types::routes::HotTagsInputDTO;
use nexus_common::types::{StreamReach, Timeframe, WotDepth};

const SUBJECT: &str = "audit_subject";
const VIEWER: &str = "audit_viewer";
const HOT_POST: &str = "audit_post_1";
const RESOURCE: &str = "audit_resource";
const LABEL: &str = "audit_label";

/// Emit one profiled query, delimited for the runner to split on.
fn emit(label: &str, q: Query) {
    let cypher = q.to_cypher_populated();
    let body = cypher.trim().trim_end_matches(';');
    println!("@@QUERY {label}");
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

fn main() {
    // Counts (the #935 class).
    emit("user_counts", get::user_counts(SUBJECT));
    emit("post_counts", get::post_counts(SUBJECT, HOT_POST));

    // Details / relationships.
    emit("get_post_by_id", get::get_post_by_id(SUBJECT, HOT_POST));
    emit(
        "post_relationships",
        get::post_relationships(SUBJECT, HOT_POST),
    );
    emit(
        "get_users_details_by_ids",
        get::get_users_details_by_ids(&[SUBJECT, VIEWER]),
    );

    // Followers / following.
    emit(
        "get_user_followers",
        get::get_user_followers(SUBJECT, Some(0), Some(30)),
    );
    emit(
        "get_user_following",
        get::get_user_following(SUBJECT, Some(0), Some(30)),
    );

    // Tags.
    emit("get_post_tags", get::get_post_tags(SUBJECT, HOT_POST));
    emit("post_tags", get::post_tags(SUBJECT, HOT_POST));
    emit("user_tags", get::user_tags(SUBJECT));

    // Notification fan-out reads.
    emit(
        "get_post_bookmarks",
        get::get_post_bookmarks(SUBJECT, HOT_POST),
    );
    emit("get_post_reposts", get::get_post_reposts(SUBJECT, HOT_POST));
    emit("get_post_replies", get::get_post_replies(SUBJECT, HOT_POST));
    emit("user_bookmarks", get::user_bookmarks(SUBJECT));

    // Recommend.
    emit("recommend_users", get::recommend_users(SUBJECT, 10));

    // Influencers (global = OPTIONAL MATCH fan-out; reach = CALL{} subqueries).
    emit(
        "get_global_influencers",
        get::get_global_influencers(0, 20, &Timeframe::AllTime),
    );
    emit(
        "get_influencers_by_reach",
        get::get_influencers_by_reach(SUBJECT, StreamReach::Followers, 0, 20, &Timeframe::AllTime),
    );

    // Hot tags + taggers by reach.
    emit(
        "get_global_hot_tags",
        get::get_global_hot_tags(&hot_tags_dto()),
    );
    emit(
        "get_hot_tags_by_reach",
        get::get_hot_tags_by_reach(SUBJECT, StreamReach::Followers, &hot_tags_dto()),
    );
    emit(
        "get_tag_taggers_by_reach",
        get::get_tag_taggers_by_reach(LABEL, SUBJECT, StreamReach::Followers, 0, 30),
    );

    // Web of Trust tag aggregation.
    emit(
        "get_viewer_trusted_network_tags",
        get::get_viewer_trusted_network_tags(SUBJECT, VIEWER, WotDepth::default()),
    );

    // Resource.
    emit("get_resource_by_id", get::get_resource_by_id(RESOURCE));
}
