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
//! | Post, `t.id` matches the `pubky.app` post | Computed from the v1 post URI |
//! | Post, no match (a `social/v1` address, versioned or not) | Kept |
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
//! a user edge recomputes the v0 id for the `pubky.app` spelling, with no I/O, because the edge
//! no longer holds which one the tagger wrote: a tagger still on `pubky.app` may have tagged a
//! post by its `social/v1` address, versioned or not, which the migrator keeps as it is and
//! which the keys cannot rebuild, so the id stays.
//!
//! Paging never uses `SKIP`: each loop reads up to a batch of edges still under the `pubky.app`
//! prefix, with no offset, and each write moves its edges out of that set. Every row is
//! rewritten, kept and logged ones included, so the loop ends and a rerun picks up what is
//! left. As a guard, a read that returns rows whose write changes none fails the batch with
//! an error, so it goes through the normal retry instead of reading the same rows forever; a
//! write that changes fewer rows than were read is fine, another loop may have rewritten or
//! deleted some. Each batch is its own transaction.

use super::attachments;
use super::files::FileMap;
use crate::events::EventProcessorError;
use nexus_common::db::{fetch_all_rows_from_graph, fetch_key_from_graph, queries, GraphError};
use pubky_social_specs::legacy_v0::traits::HashId as _;
use pubky_social_specs::legacy_v0::{
    post_uri_builder as v0_post_uri_builder, tag_uri_builder as v0_tag_uri_builder,
    user_uri_builder as v0_user_uri_builder, ParsedUri, PubkyAppTag, Resource,
};
use pubky_social_specs::traits::HashId;
use pubky_social_specs::{
    post_uri_builder, tag_uri_builder, user_uri_builder, PubkyId, PubkySocialTag, PROTOCOL,
};
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
    loop {
        let tags = read_page(tagger_id, &prefix, batch_size).await?;
        if tags.is_empty() {
            return Ok(());
        }
        let rows: Vec<(String, String, String)> = tags
            .iter()
            .map(|tag| {
                let (id, uri) = rewrite(user_id, files, tag);
                (tag.edge.clone(), id, uri)
            })
            .collect();
        let changed = fetch_key_from_graph::<i64>(
            queries::migration::set_tag_addresses(tagger_id, &rows),
            "changed",
        )
        .await?
        .unwrap_or(0);
        if changed == 0 {
            return Err(EventProcessorError::GraphQueryFailed(
                false,
                format!(
                    "none of the {} tag edges read under {prefix} were rewritten",
                    tags.len()
                ),
            ));
        }
    }
}

/// One page of the user's edges still under `prefix`.
async fn read_page(
    tagger_id: &str,
    prefix: &str,
    batch_size: usize,
) -> Result<Vec<StoredTag>, EventProcessorError> {
    let page = queries::migration::get_tagger_edges_under(tagger_id, prefix, batch_size);
    Ok(fetch_all_rows_from_graph(page)
        .await?
        .iter()
        .map(|row| {
            let required = |key: &str| row.get::<String>(key).map_err(GraphError::from);
            let target = match required("kind")?.as_str() {
                "post" => Target::Post {
                    author_id: row
                        .get::<Option<String>>("author_id")
                        .map_err(GraphError::from)?,
                    post_id: required("target_id")?,
                },
                "user" => Target::User(required("target_id")?),
                "resource" => Target::Resource(required("resource_uri")?),
                _ => Target::Other,
            };
            Ok(StoredTag {
                edge: required("edge")?,
                id: required("id")?,
                label: required("label")?,
                target,
            })
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
            let v0_uri = v0_post_uri_builder(author_id.clone(), post_id.clone());
            if tag.id == v0_tag_id(&v0_uri, &tag.label) {
                NewId::Computed(tag_id(
                    &post_uri_builder(author_id.clone(), post_id.clone()),
                    &tag.label,
                ))
            } else {
                NewId::Kept
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
    use pubky_social_specs::file_uri_builder;
    use pubky_social_specs::legacy_v0::file_uri_builder as v0_file_uri_builder;

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

    /// A tagger still on `pubky.app` tagged the post by its `social/v1` address: the migrator
    /// keeps that target, so the id computed from the versionless keys would name another file.
    #[test]
    fn a_post_tag_on_a_v1_address_keeps_its_id() {
        let versionless = post_uri_builder(OTHER.to_string(), POST_ID.to_string());
        let versioned = format!("{versionless}/0032SSN7Q4EVJ.json");
        for spelling in [versionless, versioned] {
            let tag = v0_edge(&spelling, post(OTHER));
            assert_eq!(new_id(&owner(), &files(), &tag), NewId::Kept, "{spelling}");
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
}
