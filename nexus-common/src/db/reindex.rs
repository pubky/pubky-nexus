use crate::db::graph::exec::fetch_all_rows_from_graph;
use crate::db::graph::Query;
use crate::db::kv::clear_redis;
use crate::models::follow::{Followers, Following, UserFollows};
use crate::models::post::search::PostsByTagSearch;
use crate::models::post::Bookmark;
use crate::models::tag::post::TagPost;
use crate::models::tag::search::TagSearch;
use crate::models::tag::stream::HotTags;
use crate::models::tag::traits::TagCollection;
use crate::models::tag::user::TagUser;
use crate::models::traits::Collection;
use crate::models::user::{Influencers, SocialGraphStatus, UserDetails, UsersByTagSearch};
use crate::types::DynError;
use crate::{
    models::post::{PostCounts, PostDetails, PostRelationships},
    models::user::UserCounts,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tracing::{info, Instrument};

/// Clean rebuild of the whole Redis index from the graph: flushes the logical
/// database (which re-applies the RediSearch schema), then reindexes every
/// entity via [`sync`]. Any entity that failed to reindex makes this an error:
/// after a flush, a partial index must not pass as a finished rebuild.
pub async fn rebuild() -> Result<(), DynError> {
    info!("Dropping Redis database...");
    clear_redis().await?;
    info!("Starting reindexing process...");
    sync().await
}

/// Upper bound on concurrently running entity reindex tasks. Unbounded spawning
/// works on mock-sized datasets but a production graph fans out into thousands
/// of simultaneous Cypher queries and exhausts memory on both ends.
const REINDEX_CONCURRENCY: usize = 32;

/// Reindex every entity from the graph. Per-entity failures are logged as they
/// happen and reported once at the end, so a run with one broken user is still
/// visible as a failure to the caller.
#[tracing::instrument(name = "reindex.sync", skip_all)]
pub async fn sync() -> Result<(), DynError> {
    let mut user_tasks = JoinSet::new();
    let mut post_tasks = JoinSet::new();
    let semaphore = Arc::new(Semaphore::new(REINDEX_CONCURRENCY));
    let failures = Arc::new(AtomicUsize::new(0));

    let user_ids: Vec<String> = get_all_user_ids().await?;
    let user_ids_refs: Vec<&str> = user_ids.iter().map(|id| id.as_str()).collect();

    UserDetails::reindex(&user_ids_refs).await?;
    //TODO use collections for every other model

    for user_id in user_ids {
        // Acquire before spawning so pending work queues here instead of as
        // parked tasks; live tasks (and their spans) stay capped at the bound
        let permit = semaphore.clone().acquire_owned().await?;
        let failures = failures.clone();
        let span = tracing::info_span!("reindex.user", user_id = %user_id);
        user_tasks.spawn(
            async move {
                let _permit = permit;
                if let Err(e) = reindex_user(&user_id).await {
                    tracing::error!("Failed to reindex user {}: {:?}", user_id, e);
                    failures.fetch_add(1, Ordering::Relaxed);
                }
            }
            .instrument(span),
        );
    }

    let post_ids = get_all_post_ids().await?;
    for (author_id, post_id) in post_ids {
        let permit = semaphore.clone().acquire_owned().await?;
        let failures = failures.clone();
        let span = tracing::info_span!("reindex.post", author_id = %author_id, post_id = %post_id);
        post_tasks.spawn(
            async move {
                let _permit = permit;
                if let Err(e) = reindex_post(&author_id, &post_id).await {
                    tracing::error!("Failed to reindex post {}: {:?}", post_id, e);
                    failures.fetch_add(1, Ordering::Relaxed);
                }
            }
            .instrument(span),
        );
    }

    while let Some(res) = user_tasks.join_next().await {
        if let Err(e) = res {
            tracing::error!("User reindexing task failed: {:?}", e);
            failures.fetch_add(1, Ordering::Relaxed);
        }
    }

    while let Some(res) = post_tasks.join_next().await {
        if let Err(e) = res {
            tracing::error!("Post reindexing task failed: {:?}", e);
            failures.fetch_add(1, Ordering::Relaxed);
        }
    }

    HotTags::reindex().await?;
    Influencers::reindex().await?;
    SocialGraphStatus::reindex().await?;
    PostsByTagSearch::reindex().await?;
    UsersByTagSearch::reindex().await?;
    TagSearch::reindex().await?;

    let failed = failures.load(Ordering::Relaxed);
    if failed > 0 {
        return Err(
            format!("Reindexing finished with {failed} failed entities, see the log").into(),
        );
    }
    info!("Reindexing completed successfully.");
    Ok(())
}

pub async fn reindex_user(user_id: &str) -> Result<(), DynError> {
    tokio::try_join!(
        Bookmark::reindex(user_id),
        UserCounts::reindex(user_id),
        Followers::reindex(user_id),
        Following::reindex(user_id),
        TagUser::reindex(user_id, None)
    )?;
    Ok(())
}

pub async fn reindex_post(author_id: &str, post_id: &str) -> Result<(), DynError> {
    tokio::try_join!(
        PostDetails::reindex(author_id, post_id),
        PostCounts::reindex(author_id, post_id),
        PostRelationships::reindex(author_id, post_id),
        TagPost::reindex(author_id, Some(post_id))
    )?;
    Ok(())
}

pub async fn get_all_user_ids() -> Result<Vec<String>, DynError> {
    let query = Query::new("get_all_user_ids", "MATCH (u:User) RETURN u.id AS id");
    let rows = fetch_all_rows_from_graph(query).await?;

    let mut user_ids = Vec::new();
    for row in rows {
        if let Some(id) = row.get("id")? {
            user_ids.push(id);
        }
    }

    Ok(user_ids)
}

async fn get_all_post_ids() -> Result<Vec<(String, String)>, DynError> {
    let query = Query::new(
        "get_all_post_ids",
        "MATCH (u:User)-[:AUTHORED]->(p:Post) RETURN u.id AS author_id, p.id AS post_id",
    );
    let rows = fetch_all_rows_from_graph(query).await?;

    let mut post_ids = Vec::new();
    for row in rows {
        if let (Some(author_id), Some(post_id)) = (row.get("author_id")?, row.get("post_id")?) {
            post_ids.push((author_id, post_id));
        }
    }

    Ok(post_ids)
}
