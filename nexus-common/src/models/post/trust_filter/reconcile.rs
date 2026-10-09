//! Bringing the shared sets in line with a published ranking.

use std::collections::{BTreeMap, BTreeSet};

use deadpool_redis::Connection;
use neo4rs::Row;
use redis::{AsyncCommands, Pipeline, Script};

use super::scripts::{add_call, remove_call, ADD, REMOVE};
use super::{
    admitted, applied_key, is_enabled, is_ranking_applied, sorted_key, APPLIED_KEY_PARTS, BATCH,
};
use crate::db::kv::{RedisError, RedisResult};
use crate::db::queries::get::PostEntries;
use crate::db::{
    fetch_all_rows_from_graph, fetch_key_from_graph, get_redis_conn, queries, GraphResult, RedisOps,
};
use crate::models::error::ModelResult;
use crate::models::post::search::PostsByTagSearch;
use crate::models::post::{PostCounts, PostStream};
use crate::models::user::SocialGraphStatus;

/// Posts read from the graph, and written, per page.
const POSTS_PAGE: usize = 1_000;

/// Brings the shared sets in line with the published ranking, then records it
/// as applied: the changed authors' posts go out or back, and the posts they
/// engaged with are rescored. With the filter off the ranking counts as absent,
/// which writes every hidden author back. Takes no lock: [`REMOVE`] spares an author the
/// live ranking admits and [`ADD`] skips one it doesn't, so overlapping runs
/// converge.
pub(crate) async fn reconcile() -> ModelResult<()> {
    let mut conn = get_redis_conn().await?;
    let ranking = match is_enabled() {
        true => members(&mut conn, &SocialGraphStatus::ranking_key()).await?,
        false => None,
    };
    let applied = members(&mut conn, &applied_key()).await?;
    if applied == ranking {
        return Ok(());
    }

    let (hide, show) = rank_changes(applied.as_ref(), ranking.as_ref()).await?;
    tracing::info!(
        hide = hide.len(),
        show = show.len(),
        "Applying a rank change to the shared post sets"
    );
    let hides = hide.iter().map(|author| (author, Write::Hide));
    let shows = show.iter().map(|author| (author, Write::Show));
    for (author, write) in hides.chain(shows) {
        let pages = [
            (PostEntries::WrittenBy(author), write),
            (PostEntries::EngagedBy(author), Write::Rescore),
        ];
        for (posts, write) in pages {
            write_pages(&mut conn, posts, write, POSTS_PAGE)
                .await
                .inspect_err(|error| {
                    tracing::error!(author, ?write, %error, "Failed to update an author's shared post entries")
                })?;
        }
    }
    // Last, so a run that fails keeps the old copy and the next one redoes the diff.
    record_applied(ranking.as_ref()).await?;
    tracing::info!("Rank change applied to the shared post sets");
    Ok(())
}

/// Rescores the engagement of every post in the shared sets, counting only the
/// engagers the filter admits: a full reindex counts everyone's.
pub(crate) async fn rescore_all() -> ModelResult<()> {
    if !is_ranking_applied().await? {
        return Ok(());
    }
    let mut conn = get_redis_conn().await?;
    write_pages(&mut conn, PostEntries::All, Write::Rescore, POSTS_PAGE).await
}

/// The authors to hide and to show when the shared sets go from the `applied`
/// ranking to `ranking`. No ranking takes every author.
async fn rank_changes(
    applied: Option<&BTreeSet<String>>,
    ranking: Option<&BTreeSet<String>>,
) -> ModelResult<(Vec<String>, Vec<String>)> {
    Ok(match (applied, ranking) {
        (None, None) => (Vec::new(), Vec::new()),
        // The first ranking: until now the sets took everyone.
        (None, Some(ranking)) => (authors_outside(ranking).await?, Vec::new()),
        (Some(applied), Some(ranking)) => (
            applied.difference(ranking).cloned().collect(),
            ranking.difference(applied).cloned().collect(),
        ),
        // The ranking is gone: the sets take everyone again.
        (Some(applied), None) => (Vec::new(), authors_outside(applied).await?),
    })
}

/// Records `ranking` as the one the shared sets reflect. `None` deletes the
/// copy, so nothing reads as filtered.
async fn record_applied(ranking: Option<&BTreeSet<String>>) -> RedisResult<()> {
    let applied: Vec<(f64, &str)> = ranking
        .into_iter()
        .flatten()
        .map(|id| (0.0, id.as_str()))
        .collect();
    PostStream::replace_index_sorted_set(&APPLIED_KEY_PARTS, &applied, None, None).await
}

/// The members of the sorted set `key`, or `None` when it doesn't exist.
async fn members(conn: &mut Connection, key: &str) -> RedisResult<Option<BTreeSet<String>>> {
    let members: BTreeSet<String> = conn.zrange(key, 0, -1).await?;
    Ok((!members.is_empty()).then_some(members))
}

/// Every user who wrote a post, ids in `set` aside.
async fn authors_outside(set: &BTreeSet<String>) -> ModelResult<Vec<String>> {
    let query = queries::get::post_author_ids();
    let mut authors: Vec<String> = fetch_key_from_graph(query, "user_ids")
        .await?
        .unwrap_or_default();
    authors.retain(|author| !set.contains(author));
    Ok(authors)
}

#[derive(Debug, Clone, Copy)]
pub(super) enum Write {
    /// Remove the author's entries, unless the live ranking admits them.
    Hide,
    /// Write them back where the live ranking admits them, or everywhere
    /// with the filter off.
    Show,
    /// Update the scores of the entries already there, and drop the posts'
    /// cached counts.
    Rescore,
}

impl Write {
    fn script(self) -> Option<&'static Script> {
        match self {
            Write::Hide => Some(&REMOVE),
            Write::Show => Some(&ADD),
            Write::Rescore => None,
        }
    }

    /// Queues the call that writes, or removes, `batch` in the shared set `key`.
    fn queue(self, pipe: &mut Pipeline, key: &str, batch: &[(f64, String)]) {
        let entries = batch
            .iter()
            .map(|(score, member)| (*score, member.as_str()));
        match self {
            Write::Hide => pipe.invoke_script(&remove_call(key, entries.map(|(_, member)| member))),
            Write::Show => pipe.invoke_script(&add_call(key, !is_enabled(), entries)),
            Write::Rescore => {
                pipe.cmd("ZADD").arg(key).arg("XX");
                for (score, member) in entries {
                    pipe.arg(score).arg(member);
                }
                pipe
            }
        }
        .ignore();
    }
}

/// Applies `write` to the selected posts' entries `page_size` posts at a time,
/// oldest first, so a prolific author needs no single huge query or pipeline.
pub(super) async fn write_pages(
    conn: &mut Connection,
    posts: PostEntries<'_>,
    write: Write,
    page_size: usize,
) -> ModelResult<()> {
    let mut after = (i64::MIN, String::new());
    loop {
        let query = queries::get::post_entries(posts, (after.0, &after.1), page_size);
        let rows = fetch_all_rows_from_graph(query).await?;
        let mut page: Vec<AuthorPost> = rows
            .iter()
            .map(AuthorPost::from_row)
            .collect::<Result<_, _>>()?;
        count_engagement(&mut page).await?;
        write_entries(conn, write, &shared_entries(&page)).await?;
        if let Write::Rescore = write {
            let keys: Vec<[&str; 2]> = page
                .iter()
                .map(|post| [post.author.as_str(), &post.id])
                .collect();
            let keys: Vec<&[&str]> = keys.iter().map(|key| key.as_slice()).collect();
            PostCounts::invalidate_many(&keys).await?;
        }
        match page.iter().map(|post| (post.indexed_at, &post.id)).max() {
            Some((indexed_at, id)) if page.len() >= page_size => after = (indexed_at, id.clone()),
            _ => return Ok(()),
        }
    }
}

/// Applies `write` to `entries` in one pipeline of script calls.
async fn write_entries(
    conn: &mut Connection,
    write: Write,
    entries: &BTreeMap<String, Vec<(f64, String)>>,
) -> ModelResult<()> {
    if entries.is_empty() {
        return Ok(());
    }
    let mut pipe = redis::pipe();
    if let Some(script) = write.script() {
        pipe.load_script(script).ignore();
    }
    for (key, entries) in entries {
        for batch in entries.chunks(BATCH) {
            write.queue(&mut pipe, key, batch);
        }
    }
    let _: () = pipe.query_async(conn).await.map_err(RedisError::from)?;
    Ok(())
}

/// A post, with what the shared sets index it by.
#[derive(Debug)]
pub(crate) struct AuthorPost {
    pub author: String,
    pub id: String,
    pub indexed_at: i64,
    /// `(author, post)` of the post it replies to.
    pub parent: Option<(String, String)>,
    /// The parent post's author follows this post's author.
    pub followed_by_parent_author: bool,
    /// The labels it is tagged with.
    pub labels: Vec<String>,
    /// The user behind each tag, reply and repost.
    pub engagers: Vec<String>,
    /// Tags, replies and reposts that count: see [`count_engagement`].
    pub engagement: i64,
    /// Users it mentions.
    pub mentions: i64,
}

impl AuthorPost {
    pub(super) fn from_row(row: &Row) -> GraphResult<Self> {
        let parent_author: Option<String> = row.get("parent_author_id")?;
        let parent_post: Option<String> = row.get("parent_post_id")?;
        let engagers: Vec<String> = row.get("engagers")?;
        Ok(AuthorPost {
            author: row.get("author_id")?,
            id: row.get("post_id")?,
            indexed_at: row.get("indexed_at")?,
            parent: parent_author.zip(parent_post),
            followed_by_parent_author: row.get("followed_by_parent_author")?,
            labels: row.get("labels")?,
            engagement: engagers.len() as i64,
            engagers,
            mentions: row.get("mentions")?,
        })
    }
}

/// Counts each post's engagement from the engagers the filter admits, and the
/// post's own author.
pub(super) async fn count_engagement(posts: &mut [AuthorPost]) -> RedisResult<()> {
    let engagers: BTreeSet<String> = posts
        .iter()
        .flat_map(|post| post.engagers.clone())
        .collect();
    let engagers: Vec<&str> = engagers.iter().map(String::as_str).collect();
    let admitted: BTreeSet<&str> = engagers
        .iter()
        .zip(admitted(&engagers).await?)
        .filter_map(|(user, admitted)| admitted.then_some(*user))
        .collect();
    for post in posts {
        let counted = post
            .engagers
            .iter()
            .filter(|user| **user == post.author || admitted.contains(user.as_str()));
        post.engagement = counted.count() as i64;
    }
    Ok(())
}

/// Every entry the posts have in the shared sets, by key, as the models that
/// own those sets place them.
pub(super) fn shared_entries(posts: &[AuthorPost]) -> BTreeMap<String, Vec<(f64, String)>> {
    let mut entries: BTreeMap<String, Vec<(f64, String)>> = BTreeMap::new();
    for post in posts {
        let member = format!("{}:{}", post.author, post.id);
        let sets = PostStream::shared_set_entries(post)
            .into_iter()
            .chain(PostsByTagSearch::shared_set_entries(post));
        for (key_parts, score) in sets {
            let key = sorted_key(&[&key_parts]);
            entries
                .entry(key)
                .or_default()
                .push((score, member.clone()));
        }
    }
    entries
}
