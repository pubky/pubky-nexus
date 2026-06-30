use crate::db::graph::exec::fetch_all_rows_from_graph;
use crate::db::graph::Query;
use crate::models::follow::{Followers, Following, UserFollows};
use crate::models::post::search::PostsByTagSearch;
use crate::models::post::Bookmark;
use crate::models::tag::post::TagPost;
use crate::models::tag::search::TagSearch;
use crate::models::tag::stream::HotTags;
use crate::models::tag::traits::TagCollection;
use crate::models::tag::user::TagUser;
use crate::models::traits::Collection;
use crate::models::user::{Influencers, UserDetails};
use crate::types::DynError;
use crate::{
    models::post::{PostCounts, PostDetails, PostRelationships},
    models::user::UserCounts,
};
use std::future::Future;
use tokio::task::JoinSet;
use tracing::{info, Instrument};

pub const REINDEX_BATCH_SIZE: usize = 500;

/// Drives a keyset-paginated scan over graph rows.
///
/// `step(cursor)` fetches one batch starting after `cursor` and returns
/// `(count, last_cursor)`.  The loop stops when `count < batch_size` or
/// the batch is empty.  If a full batch yields no cursor the scan is
/// aborted with `Err`.
pub async fn keyset_scan<F, Fut, E>(batch_size: usize, context: &str, mut step: F) -> Result<(), E>
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = Result<(usize, Option<String>), E>>,
    E: From<String>,
{
    let mut cursor = String::new();
    loop {
        let (count, last_cursor) = step(cursor).await?;
        if count == 0 || count < batch_size {
            break;
        }
        cursor = last_cursor.ok_or_else(|| {
            E::from(format!(
                "{context}: batch of {count} rows produced no usable cursor"
            ))
        })?;
    }
    Ok(())
}

#[tracing::instrument(name = "reindex.sync", skip_all)]
pub async fn sync() {
    reindex_users_by_batch()
        .await
        .expect("Failed to reindex users by batch");
    reindex_posts_by_batch()
        .await
        .expect("Failed to reindex posts by batch");

    HotTags::reindex()
        .await
        .expect("Failed to store the global hot tags");

    Influencers::reindex()
        .await
        .expect("Failed to reindex influencers");

    PostsByTagSearch::reindex()
        .await
        .expect("Failed to store the global post tags");

    TagSearch::reindex()
        .await
        .expect("Failed to store the global tags");

    info!("Reindexing completed successfully.");
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

async fn reindex_users_by_batch() -> Result<(), DynError> {
    let query = Query::new(
        "get_all_user_ids",
        "MATCH (u:User) WHERE u.id > $cursor RETURN u.id AS id ORDER BY u.id LIMIT $limit",
    );
    keyset_scan(REINDEX_BATCH_SIZE, "reindex_users", |cursor| {
        let query = query.clone();
        async move {
            let batch = fetch_all_rows_from_graph(
                query
                    .param("cursor", cursor.as_str())
                    .param("limit", REINDEX_BATCH_SIZE as i64),
            )
            .await?;
            let count = batch.len();
            let mut user_ids: Vec<String> = Vec::with_capacity(count);
            let mut last_id = String::new();
            for row in batch {
                match row.get::<String>("id") {
                    Ok(id) => {
                        last_id = id.clone();
                        user_ids.push(id);
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Failed to extract user id from graph row");
                    }
                }
            }
            if !user_ids.is_empty() {
                {
                    let user_ids_refs: Vec<&str> = user_ids.iter().map(String::as_str).collect();
                    UserDetails::reindex(&user_ids_refs).await?;
                }
                let mut tasks = JoinSet::new();
                for user_id in user_ids {
                    let span = tracing::info_span!("reindex.user", user_id = %user_id);
                    tasks.spawn(
                        async move {
                            if let Err(e) = reindex_user(&user_id).await {
                                tracing::error!("Failed to reindex user {}: {:?}", user_id, e);
                            }
                        }
                        .instrument(span),
                    );
                }
                while let Some(res) = tasks.join_next().await {
                    if let Err(e) = res {
                        tracing::error!("User reindexing task failed: {:?}", e);
                    }
                }
            }
            Ok((
                count,
                if last_id.is_empty() {
                    None
                } else {
                    Some(last_id)
                },
            ))
        }
    })
    .await
}

async fn reindex_posts_by_batch() -> Result<(), DynError> {
    let query = Query::new(
        "get_all_post_ids",
        "MATCH (u:User)-[:AUTHORED]->(p:Post) WHERE p.id > $cursor RETURN u.id AS author_id, p.id AS post_id ORDER BY p.id LIMIT $limit",
    );
    keyset_scan(REINDEX_BATCH_SIZE, "reindex_posts", |cursor| {
        let query = query.clone();
        async move {
            let batch = fetch_all_rows_from_graph(
                query
                    .param("cursor", cursor.as_str())
                    .param("limit", REINDEX_BATCH_SIZE as i64),
            )
            .await?;
            let count = batch.len();
            let mut post_ids: Vec<(String, String)> = Vec::with_capacity(count);
            let mut last_id = String::new();
            for row in batch {
                let author = match row.get::<String>("author_id") {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::warn!(error = %e, "Failed to extract author_id from graph row");
                        continue;
                    }
                };
                let post = match row.get::<String>("post_id") {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::warn!(error = %e, "Failed to extract post_id from graph row");
                        continue;
                    }
                };
                last_id = post.clone();
                post_ids.push((author, post));
            }
            if !post_ids.is_empty() {
                let mut tasks = JoinSet::new();
                for (author_id, post_id) in post_ids {
                    let span = tracing::info_span!(
                        "reindex.post",
                        author_id = %author_id,
                        post_id = %post_id
                    );
                    tasks.spawn(
                        async move {
                            if let Err(e) = reindex_post(&author_id, &post_id).await {
                                tracing::error!("Failed to reindex post {}: {:?}", post_id, e);
                            }
                        }
                        .instrument(span),
                    );
                }
                while let Some(res) = tasks.join_next().await {
                    if let Err(e) = res {
                        tracing::error!("Post reindexing task failed: {:?}", e);
                    }
                }
            }
            Ok((
                count,
                if last_id.is_empty() {
                    None
                } else {
                    Some(last_id)
                },
            ))
        }
    })
    .await
}
