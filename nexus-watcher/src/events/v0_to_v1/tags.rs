//! The tags step. Every `TAGGED` edge the user wrote whose address is under
//! `<user>/pub/pubky.app/tags/` moves to the tag file the migrator wrote, whatever its
//! target: `t.uri` becomes `pubky://<user>/pub/social/v1/tags/<t.id>.json`, built from the
//! final `t.id`, so the client's untag at the v1 address finds the edge. An edge is never left
//! at `pubky.app`: the client would untag there, and the gate would ignore the `DEL`. Edges in
//! other apps' folders and other users' edges are not touched, and labels are not rewritten.
//!
//! `t.id` is the id of the tagger's file, `blake3("{target}:{label}")` through the crate's tag
//! `HashId`, so it follows the edge's target as the migrator rewrote it (see [`new_id`]):
//!
//! | Edge target | `t.id` |
//! |---|---|
//! | Post, `t.id` matches the versionless `social/v1` post | Kept |
//! | Post, any other spelling | Computed from the v1 post URI |
//! | User, `t.id` matches `profile.json` | Computed from the v1 profile URI |
//! | User, `t.id` matches bare `pubky://pk` | Kept |
//! | User, no match | Kept, and logged |
//! | Resource, own file in the file map | Computed from `{hash}.{ext}` |
//! | Resource, anything else | Kept |
//! | Anything else, such as a post with no `AUTHORED` author | Kept, and logged |
//!
//! Post and user targets are built from the keys, never read from `Post.uri`: an unmigrated
//! author's `Post.uri` is still `pubky.app`, and the migrator rewrites another user's post or
//! profile to its v1 URI whether or not they migrated. A kept id is right because the migrator
//! keeps those targets too, so the hash input is the same; for a URL with a fragment or
//! userinfo it is the only answer, because `Resource.uri` has lost them. The match on a post or
//! a user edge recomputes the v0 id for a spelling, with no I/O, because the edge no longer
//! holds which one the tagger wrote. A tagger still on `pubky.app` may have tagged a post by
//! its versionless `social/v1` address, which the migrator keeps as it is, so that id stays;
//! every other spelling of the post (canonical, loose, or a `social/v1` version, for which the
//! migrator writes no tag file at all) takes the id of the versionless v1 URI, the id a real
//! v1 tag on that post with that label carries.
//!
//! Paging never uses `SKIP`: each loop reads up to a batch of edges still under the `pubky.app`
//! prefix, with no offset, and each write moves its edges out of that set. Every row is
//! rewritten, kept and logged ones included, so the loop ends and a rerun picks up what is
//! left. As a guard, a read that returns rows whose write changes none fails the batch with
//! an error, so it goes through the normal retry instead of reading the same rows forever; a
//! write that changes fewer rows than were read is fine, another loop may have rewritten or
//! deleted some. Each batch is its own transaction. The stop, continue or fail decision is
//! [`drain`]'s, apart from the two graph calls, so it is tested on canned pages.
//!
//! Before a batch's write, the address each `Resource` edge moves from is held for the
//! resource changes feed, which published the edge under it: see
//! [`super::resource_changes`].

use super::attachments;
use super::files::FileMap;
use super::resource_changes;
use crate::events::EventProcessorError;
use nexus_common::db::{fetch_all_rows_from_graph, fetch_key_from_graph, queries, GraphError};
use pubky_social_specs::legacy_v0::traits::HashId as _;
use pubky_social_specs::legacy_v0::{
    tag_uri_builder as v0_tag_uri_builder, user_uri_builder as v0_user_uri_builder, ParsedUri,
    PubkyAppTag, Resource,
};
use pubky_social_specs::traits::HashId;
use pubky_social_specs::{
    post_uri_builder, tag_uri_builder, user_uri_builder, PubkyId, PubkySocialTag, PROTOCOL,
};
use std::future::Future;
use tracing::{debug, warn};

/// Edges per batch: one graph read and one graph write each.
pub const BATCH_SIZE: usize = 100;

/// A `TAGGED` edge as the page reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredTag {
    /// The edge's element id, which the write addresses it by.
    pub edge: String,
    pub id: String,
    pub label: String,
    pub target: Target,
}

/// What a `TAGGED` edge points at, by the keys the id is computed from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A post; `author_id` is `None` when it has no `AUTHORED` author.
    Post {
        author_id: Option<String>,
        post_id: String,
    },
    User(String),
    /// A `Resource`, by its stored (normalized) address.
    Resource(String),
    /// Anything else the graph may hold.
    Other,
}

/// Where an edge's id comes from after the flip.
#[derive(Debug, PartialEq, Eq)]
enum NewId {
    /// Hashed from the target's v1 address.
    Computed(String),
    /// Kept: the migrator keeps the target too, so the hash input is the same.
    Kept,
    /// Kept, with nothing to say it is right: logged, with the reason.
    Unmatched(&'static str),
}

/// Migrates the tags `user_id` wrote, in batches of `batch_size`, each batch its own
/// transaction. Running it again gives the same result.
pub async fn migrate(
    user_id: &PubkyId,
    files: &FileMap,
    batch_size: usize,
) -> Result<(), EventProcessorError> {
    debug!(%user_id, "Migrating tags");
    let tagger_id: &str = user_id.as_ref();
    // The v0 tag address with no id: everything the user wrote under `pubky.app/tags/`
    let prefix = v0_tag_uri_builder(tagger_id.to_string(), String::new());
    let prefix = prefix.as_str();
    drain(prefix, move || async move {
        let tags = read_page(tagger_id, prefix, batch_size).await?;
        if tags.is_empty() {
            return Ok((0, 0));
        }
        let mut moves = Vec::new();
        let rows: Vec<(String, String, String)> = tags
            .iter()
            .map(|(tag, from)| {
                let (id, uri) = rewrite(user_id, files, tag);
                if let (Target::Resource(resource_uri), Some(from)) = (&tag.target, from) {
                    moves.push(resource_changes::Move {
                        resource_id: from.resource_id.clone(),
                        resource_uri: resource_uri.clone(),
                        label: tag.label.clone(),
                        from: from.tag_uri.clone(),
                        to: uri.clone(),
                    });
                }
                (tag.edge.clone(), id, uri)
            })
            .collect();
        // Before the write, which takes the old address off the edge for good
        resource_changes::prepare(user_id, &moves).await?;
        let changed = fetch_key_from_graph::<i64>(
            queries::migration::set_tag_addresses(tagger_id, &rows),
            "changed",
        )
        .await?
        .unwrap_or(0);
        Ok((tags.len(), changed))
    })
    .await
}

/// Drives the drain of the edges under `prefix`: `step()` reads one page and rewrites it,
/// returning how many edges it read and how many the write changed. An empty page ends the
/// drain; a page whose write changed nothing fails it with a retryable graph error, so the flip
/// goes through the normal retry instead of reading the same rows forever. Fewer changed than
/// read is fine.
async fn drain<F, Fut>(prefix: &str, mut step: F) -> Result<(), EventProcessorError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<(usize, i64), EventProcessorError>>,
{
    loop {
        let (read, changed) = step().await?;
        if read == 0 {
            return Ok(());
        }
        if changed == 0 {
            return Err(EventProcessorError::GraphQueryFailed(
                false,
                format!("none of the {read} tag edges read under {prefix} were rewritten"),
            ));
        }
    }
}

/// Where a `Resource` edge of a page is before the rewrite, for the resource changes feed.
#[derive(Debug, Clone, PartialEq, Eq)]
struct MovedFrom {
    resource_id: String,
    tag_uri: String,
}

/// One page of the user's edges still under `prefix`, each `Resource` edge with where it is.
async fn read_page(
    tagger_id: &str,
    prefix: &str,
    batch_size: usize,
) -> Result<Vec<(StoredTag, Option<MovedFrom>)>, EventProcessorError> {
    let page = queries::migration::get_tagger_edges_under(tagger_id, prefix, batch_size);
    Ok(fetch_all_rows_from_graph(page)
        .await?
        .iter()
        .map(|row| {
            let required = |key: &str| row.get::<String>(key).map_err(GraphError::from);
            let mut from = None;
            let target = match required("kind")?.as_str() {
                "post" => Target::Post {
                    author_id: row
                        .get::<Option<String>>("author_id")
                        .map_err(GraphError::from)?,
                    post_id: required("target_id")?,
                },
                "user" => Target::User(required("target_id")?),
                "resource" => {
                    from = Some(MovedFrom {
                        resource_id: required("target_id")?,
                        tag_uri: required("tag_uri")?,
                    });
                    Target::Resource(required("resource_uri")?)
                }
                _ => Target::Other,
            };
            let tag = StoredTag {
                edge: required("edge")?,
                id: required("id")?,
                label: required("label")?,
                target,
            };
            Ok((tag, from))
        })
        .collect::<Result<_, GraphError>>()?)
}

/// The id and the address `tag` takes in `owner`'s migrated graph. An id kept with nothing to
/// say it is right is logged here, with the user, the edge and the reason.
pub fn rewrite(owner: &PubkyId, files: &FileMap, tag: &StoredTag) -> (String, String) {
    let id = match new_id(owner, files, tag) {
        NewId::Computed(id) => id,
        NewId::Kept => tag.id.clone(),
        NewId::Unmatched(reason) => {
            warn!(
                user_id = %owner,
                tag_id = %tag.id,
                label = %tag.label,
                target = ?tag.target,
                "Tag id kept as it is: {reason}"
            );
            tag.id.clone()
        }
    };
    let uri = tag_uri_builder(owner.to_string(), id.clone());
    (id, uri)
}

/// The table in the module docs, one row per target.
fn new_id(owner: &PubkyId, files: &FileMap, tag: &StoredTag) -> NewId {
    match &tag.target {
        Target::Post {
            author_id: Some(author_id),
            post_id,
        } => {
            let v1_uri = post_uri_builder(author_id.clone(), post_id.clone());
            if tag.id == v0_tag_id(&v1_uri, &tag.label) {
                NewId::Kept
            } else {
                NewId::Computed(tag_id(&v1_uri, &tag.label))
            }
        }
        Target::Post {
            author_id: None, ..
        } => NewId::Unmatched("the tagged post has no author"),
        Target::User(user_id) => {
            if tag.id == v0_tag_id(&v0_user_uri_builder(user_id.clone()), &tag.label) {
                NewId::Computed(tag_id(&user_uri_builder(user_id.clone()), &tag.label))
            } else if tag.id == v0_tag_id(&[PROTOCOL, user_id].concat(), &tag.label) {
                NewId::Kept
            } else {
                NewId::Unmatched("no spelling of the tagged user reproduces the id")
            }
        }
        Target::Resource(uri) => match own_file_hash(owner, files, uri) {
            Some(hash) => NewId::Computed(tag_id(
                &attachments::migrated_file_uri(owner, files, hash),
                &tag.label,
            )),
            None => NewId::Kept,
        },
        Target::Other => NewId::Unmatched("the target is neither a post, a user nor a resource"),
    }
}

/// The blob `uri` names, if it is `owner`'s v0 file and the file map knows it.
fn own_file_hash<'a>(owner: &PubkyId, files: &'a FileMap, uri: &str) -> Option<&'a str> {
    let parsed = ParsedUri::try_from(uri)
        .ok()
        .filter(|p| p.user_id == *owner)?;
    match parsed.resource {
        Resource::File(tsid) => files.hash_of(&tsid),
        _ => None,
    }
}

/// The id of the v1 tag file on `target` with `label`, through the crate's tag `HashId`.
fn tag_id(target: &str, label: &str) -> String {
    PubkySocialTag {
        uri: target.to_string(),
        label: label.to_string(),
        created_at: 0,
        extra: Default::default(),
    }
    .create_id()
}

/// The id of the v0 tag file on `target` with `label`, through the legacy tag `HashId`.
fn v0_tag_id(target: &str, label: &str) -> String {
    PubkyAppTag {
        uri: target.to_string(),
        label: label.to_string(),
        created_at: 0,
    }
    .create_id()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::retry::RetryScheduler;
    use pubky_social_specs::file_uri_builder;
    use pubky_social_specs::legacy_v0::{
        file_uri_builder as v0_file_uri_builder, post_uri_builder as v0_post_uri_builder,
    };
    use std::cell::RefCell;

    const OWNER: &str = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
    const OTHER: &str = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";
    const POST_ID: &str = "0032SSN7Q4EVG";
    const TSID: &str = "0032SSN7Q4EVG";
    const HASH: &str = "8Z8CWH8NVYQY39ZEBFGKQWWEKG";
    const LABEL: &str = "nexus";

    fn owner() -> PubkyId {
        PubkyId::try_from(OWNER).unwrap()
    }

    fn files() -> FileMap {
        FileMap::from_files(
            &owner(),
            [(
                TSID.to_string(),
                format!("pubky://{OWNER}/pub/pubky.app/blobs/{HASH}"),
                "image/png".to_string(),
            )],
        )
    }

    /// An edge as a `pubky.app` tag on `target` left it: its v0 id.
    fn v0_edge(target_spelling: &str, target: Target) -> StoredTag {
        StoredTag {
            edge: "4:edge:1".to_string(),
            id: v0_tag_id(target_spelling, LABEL),
            label: LABEL.to_string(),
            target,
        }
    }

    fn post(author: &str) -> Target {
        Target::Post {
            author_id: Some(author.to_string()),
            post_id: POST_ID.to_string(),
        }
    }

    #[test]
    fn a_post_tag_takes_the_id_of_the_v1_post_uri_from_the_keys() {
        for author in [OWNER, OTHER] {
            let tag = v0_edge(
                &v0_post_uri_builder(author.to_string(), POST_ID.to_string()),
                post(author),
            );
            let expected = tag_id(
                &post_uri_builder(author.to_string(), POST_ID.to_string()),
                LABEL,
            );
            assert_eq!(new_id(&owner(), &files(), &tag), NewId::Computed(expected));
        }
    }

    /// A tagger still on `pubky.app` tagged the post by its versionless `social/v1` address:
    /// the migrator keeps that target, so the id is already the v1 one.
    #[test]
    fn a_post_tag_on_the_versionless_v1_address_keeps_its_id() {
        let versionless = post_uri_builder(OTHER.to_string(), POST_ID.to_string());
        let tag = v0_edge(&versionless, post(OTHER));
        assert_eq!(new_id(&owner(), &files(), &tag), NewId::Kept);
    }

    /// Any other spelling of the post takes the id of the versionless v1 URI: a loose
    /// `pubky.app` spelling the migrator canonicalizes and rewrites, and a `social/v1` version,
    /// for which the migrator writes no tag file, so the id is the one a real v1 tag would get.
    #[test]
    fn a_post_tag_on_any_other_spelling_takes_the_computed_id() {
        let versionless = post_uri_builder(OTHER.to_string(), POST_ID.to_string());
        let expected = tag_id(&versionless, LABEL);
        let spellings = [
            format!(
                "{}/extra",
                v0_post_uri_builder(OTHER.to_string(), POST_ID.to_string())
            ),
            format!("{versionless}/0032SSN7Q4EVJ.json"),
        ];
        for spelling in spellings {
            let tag = v0_edge(&spelling, post(OTHER));
            assert_eq!(
                new_id(&owner(), &files(), &tag),
                NewId::Computed(expected.clone()),
                "{spelling}"
            );
        }
    }

    #[test]
    fn a_profile_tag_takes_the_id_of_the_v1_profile_uri() {
        let tag = v0_edge(
            &v0_user_uri_builder(OTHER.to_string()),
            Target::User(OTHER.to_string()),
        );
        let expected = tag_id(&user_uri_builder(OTHER.to_string()), LABEL);
        assert_eq!(new_id(&owner(), &files(), &tag), NewId::Computed(expected));
    }

    #[test]
    fn a_bare_user_tag_keeps_its_id() {
        let tag = v0_edge(&format!("pubky://{OTHER}"), Target::User(OTHER.to_string()));
        assert_eq!(new_id(&owner(), &files(), &tag), NewId::Kept);
    }

    #[test]
    fn a_user_tag_no_spelling_reproduces_is_unmatched() {
        let tag = StoredTag {
            id: "MADEUPTAGID".to_string(),
            ..v0_edge("", Target::User(OTHER.to_string()))
        };
        assert!(matches!(
            new_id(&owner(), &files(), &tag),
            NewId::Unmatched(_)
        ));
    }

    #[test]
    fn an_own_file_in_the_map_takes_the_id_of_its_v1_file_uri() {
        let tag = v0_edge(
            &v0_file_uri_builder(OWNER.to_string(), TSID.to_string()),
            Target::Resource(v0_file_uri_builder(OWNER.to_string(), TSID.to_string())),
        );
        let expected = tag_id(
            &file_uri_builder(OWNER.to_string(), format!("{HASH}.png")),
            LABEL,
        );
        assert_eq!(new_id(&owner(), &files(), &tag), NewId::Computed(expected));
    }

    #[test]
    fn any_other_resource_keeps_its_id() {
        let kept = [
            // Another user's file
            v0_file_uri_builder(OTHER.to_string(), TSID.to_string()),
            // An own file the map does not know
            v0_file_uri_builder(OWNER.to_string(), "0032SSN7Q4EVH".to_string()),
            // A web URL, its fragment lost by normalization
            "https://example.com/page".to_string(),
            // Another scheme, another app's path
            "nostr:npub1abc".to_string(),
            format!("pubky://{OWNER}/pub/other.app/items/1"),
        ];
        for uri in kept {
            let tag = v0_edge(&uri, Target::Resource(uri.clone()));
            assert_eq!(new_id(&owner(), &files(), &tag), NewId::Kept, "{uri}");
        }
    }

    #[test]
    fn a_post_with_no_author_and_any_other_target_are_unmatched() {
        let no_author = StoredTag {
            target: Target::Post {
                author_id: None,
                post_id: POST_ID.to_string(),
            },
            ..v0_edge("", post(OWNER))
        };
        assert!(matches!(
            new_id(&owner(), &files(), &no_author),
            NewId::Unmatched(_)
        ));
        let other = StoredTag {
            target: Target::Other,
            ..no_author
        };
        assert!(matches!(
            new_id(&owner(), &files(), &other),
            NewId::Unmatched(_)
        ));
    }

    /// The address always names the final id in the v1 tag folder, computed or kept.
    #[test]
    fn the_address_is_built_from_the_final_id() {
        let computed = v0_edge(
            &v0_post_uri_builder(OWNER.to_string(), POST_ID.to_string()),
            post(OWNER),
        );
        let (id, uri) = rewrite(&owner(), &files(), &computed);
        assert_ne!(id, computed.id);
        assert_eq!(uri, format!("pubky://{OWNER}/pub/social/v1/tags/{id}.json"));

        let kept = v0_edge(&format!("pubky://{OTHER}"), Target::User(OTHER.to_string()));
        let (id, uri) = rewrite(&owner(), &files(), &kept);
        assert_eq!(id, kept.id);
        assert_eq!(uri, format!("pubky://{OWNER}/pub/social/v1/tags/{id}.json"));
    }

    #[test]
    #[tracing_test::traced_test]
    fn an_unmatched_id_is_kept_and_logged_with_the_user_and_the_edge() {
        let tag = StoredTag {
            id: "MADEUPTAGID".to_string(),
            ..v0_edge("", Target::User(OTHER.to_string()))
        };
        let (id, uri) = rewrite(&owner(), &files(), &tag);
        assert_eq!(id, "MADEUPTAGID");
        assert_eq!(
            uri,
            format!("pubky://{OWNER}/pub/social/v1/tags/MADEUPTAGID.json")
        );
        assert!(logs_contain("Tag id kept as it is"));
        assert!(logs_contain(
            "no spelling of the tagged user reproduces the id"
        ));
        assert!(logs_contain(&format!("user_id={OWNER}")));
        assert!(logs_contain("tag_id=MADEUPTAGID"));
        assert!(logs_contain(&format!("label={LABEL}")));
    }

    #[test]
    #[tracing_test::traced_test]
    fn a_computed_or_kept_id_logs_nothing() {
        let bare = v0_edge(&format!("pubky://{OTHER}"), Target::User(OTHER.to_string()));
        rewrite(&owner(), &files(), &bare);
        let on_post = v0_edge(
            &v0_post_uri_builder(OTHER.to_string(), POST_ID.to_string()),
            post(OTHER),
        );
        rewrite(&owner(), &files(), &on_post);
        assert!(!logs_contain("Tag id kept as it is"));
    }

    /// One canned page: how many edges the read returned and how many the write changed.
    type Page = Result<(usize, i64), EventProcessorError>;

    /// Drains `pages` in order and returns the outcome with the number of steps taken.
    async fn drain_over(pages: Vec<Page>) -> (Result<(), EventProcessorError>, usize) {
        let pages = RefCell::new(pages.into_iter());
        let steps = RefCell::new(0);
        let result = drain("pubky://owner/pub/pubky.app/tags/", || {
            *steps.borrow_mut() += 1;
            let page = pages
                .borrow_mut()
                .next()
                .expect("step called more times than there are pages");
            async move { page }
        })
        .await;
        (result, steps.into_inner())
    }

    #[tokio::test]
    async fn the_drain_stops_on_an_empty_page() {
        let (result, steps) = drain_over(vec![Ok((0, 0))]).await;
        assert!(result.is_ok());
        assert_eq!(steps, 1);

        let (result, steps) = drain_over(vec![Ok((2, 2)), Ok((1, 1)), Ok((0, 0))]).await;
        assert!(result.is_ok());
        assert_eq!(steps, 3);
    }

    /// Another loop may have rewritten or deleted some of the edges the read returned.
    #[tokio::test]
    async fn the_drain_goes_on_when_fewer_edges_changed_than_were_read() {
        let (result, steps) = drain_over(vec![Ok((2, 1)), Ok((0, 0))]).await;
        assert!(result.is_ok());
        assert_eq!(steps, 2);
    }

    /// A page whose write changed nothing would be read forever: the drain fails instead, with
    /// an error the flip retries later rather than one that aborts the run.
    #[tokio::test]
    async fn the_drain_fails_into_the_retry_when_a_page_changes_nothing() {
        let (result, steps) = drain_over(vec![Ok((2, 0))]).await;
        assert_eq!(steps, 1);
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            EventProcessorError::GraphQueryFailed(false, ref message)
                if message.contains("none of the 2 tag edges")
        ));
        assert!(!err.should_not_retry_now());
        assert!(RetryScheduler::should_enqueue_related_event(&err));
    }

    #[tokio::test]
    async fn the_drain_returns_a_failed_step_as_it_is() {
        let failed = EventProcessorError::GraphQueryFailed(true, "down".to_string());
        let (result, steps) = drain_over(vec![Err(failed)]).await;
        assert_eq!(steps, 1);
        assert!(matches!(
            result,
            Err(EventProcessorError::GraphQueryFailed(true, _))
        ));
    }
}
