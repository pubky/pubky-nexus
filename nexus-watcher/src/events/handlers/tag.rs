use crate::events::retry::event::RetryEvent;
use crate::events::EventProcessorError;
use chrono::Utc;
use nexus_common::db::kv::{RedisResult, ScoreAction};
use nexus_common::db::{fetch_row_from_graph, queries, OperationOutcome, RedisOps};
use nexus_common::models::notification::Notification;
use nexus_common::models::post::search::PostsByTagSearch;
use nexus_common::models::post::PostDetails;
use nexus_common::models::post::{PostCounts, PostStream};
use nexus_common::models::resource::stream::ResourceStream;
use nexus_common::models::resource::tag::TagResource;
use nexus_common::models::resource::{classify_uri, normalize_uri, resource_id, UriCategory};
use nexus_common::models::tag::post::TagPost;
use nexus_common::models::tag::search::TagSearch;
use nexus_common::models::tag::traits::{TagCollection, TaggersCollection};
use nexus_common::models::tag::user::TagUser;
use nexus_common::models::user::UserCounts;
use nexus_common::models::user::UserDetails;
use nexus_common::types::Pagination;
use pubky_app_specs::{post_uri_builder, ParsedUri, PubkyAppTag, PubkyId, Resource};
use tracing::debug;

use super::utils::post_relationships_is_reply;

#[tracing::instrument(name = "tag.put", skip_all, fields(user_id = %tagger_id, tag_id = %tag_id))]
pub async fn sync_put(
    tag: PubkyAppTag,
    tagger_id: PubkyId,
    tag_id: String,
) -> Result<(), EventProcessorError> {
    debug!("Indexing new tag: {} -> {}", tagger_id, tag_id);

    // Parse the embeded URI to extract author_id and post_id using parse_tagged_post_uri
    let parsed_uri = ParsedUri::try_from(tag.uri.as_str()).map_err(EventProcessorError::generic)?;
    let user_id = parsed_uri.user_id;
    let indexed_at = Utc::now().timestamp_millis();

    match parsed_uri.resource {
        // If post_id is in the tagged URI, we place tag to a post.
        Resource::Post(post_id) => {
            // Place the tag on post
            put_sync_post(
                tagger_id, user_id, &post_id, &tag_id, &tag.label, &tag.uri, indexed_at,
            )
            .await
        }
        // If no post_id in the tagged URI, we place tag to a user.
        Resource::User => put_sync_user(tagger_id, user_id, &tag_id, &tag.label, indexed_at).await,
        other => Err(EventProcessorError::generic(format!(
            "The tagged resource is not Post or User, instead is: {other:?}"
        ))),
    }
}

/// Handles a tag event from an app-specific path (e.g., /pub/mapky/tags/TAG_ID).
/// Classifies the tagged URI: if it's an Internal-Known resource (Post/User), delegates
/// to the existing flow. Otherwise creates/updates a generic Resource node.
pub async fn sync_put_resource(
    tag: PubkyAppTag,
    tagger_id: PubkyId,
    tag_id: String,
    app: String,
) -> Result<(), EventProcessorError> {
    debug!(
        "Indexing resource tag: {} -> {} (app={})",
        tagger_id, tag_id, app
    );

    let category = classify_uri(&tag.uri);
    match category {
        UriCategory::InternalKnown => {
            // The tagged URI is a known Post/User — delegate to existing flow
            sync_put(tag, tagger_id, tag_id).await
        }
        UriCategory::InternalUnknown | UriCategory::External => {
            let (normalized, scheme) =
                normalize_uri(&tag.uri).map_err(EventProcessorError::generic)?;
            let res_id = resource_id(&normalized);
            let indexed_at = Utc::now().timestamp_millis();

            put_sync_resource(
                tagger_id,
                &res_id,
                &normalized,
                &scheme,
                &app,
                &tag_id,
                &tag.label,
                indexed_at,
            )
            .await
        }
    }
}

/// Creates a Resource tag in the graph and updates Redis indexes.
#[allow(clippy::too_many_arguments)]
async fn put_sync_resource(
    tagger_id: PubkyId,
    resource_id: &str,
    uri: &str,
    scheme: &str,
    app: &str,
    tag_id: &str,
    tag_label: &str,
    indexed_at: i64,
) -> Result<(), EventProcessorError> {
    match TagResource::put_to_graph_resource(
        &tagger_id,
        resource_id,
        uri,
        scheme,
        app,
        tag_id,
        tag_label,
        indexed_at,
    )
    .await?
    {
        OperationOutcome::Updated => Ok(()),
        OperationOutcome::MissingDependency => {
            // Tagger user not yet indexed
            let dependency = vec![format!("{tagger_id}")];
            Err(EventProcessorError::MissingDependency { dependency })
        }
        OperationOutcome::CreatedOrDeleted => {
            let tag_label_slice = &[tag_label.to_string()];

            let indexing_results = nexus_common::traced_join!(
                tracing::info_span!("index.write", phase = "tag_resource");
                // Update tag label score on Resource
                TagResource::update_index_score(
                    resource_id,
                    None,
                    tag_label,
                    ScoreAction::Increment(1.0),
                ),
                // Add tagger to Resource's label tagger set
                TagResource::add_tagger_to_index(resource_id, None, &tagger_id, tag_label),
                // Add to global tag search index
                TagSearch::put_to_index(tag_label_slice),
                // ResourceStream sorted set maintenance
                ResourceStream::put_to_global_timeline(resource_id, indexed_at),
                ResourceStream::update_global_taggers_count(
                    resource_id,
                    ScoreAction::Increment(1.0),
                ),
                ResourceStream::put_to_app_timeline(app, resource_id, indexed_at),
                ResourceStream::update_app_taggers_count(
                    app,
                    resource_id,
                    ScoreAction::Increment(1.0),
                ),
                ResourceStream::put_to_tag_timeline(tag_label, resource_id, indexed_at),
                ResourceStream::update_tag_taggers_count(
                    tag_label,
                    resource_id,
                    ScoreAction::Increment(1.0),
                ),
                ResourceStream::put_to_app_tag_timeline(app, tag_label, resource_id, indexed_at),
                ResourceStream::update_app_tag_taggers_count(
                    app,
                    tag_label,
                    resource_id,
                    ScoreAction::Increment(1.0),
                )
            );

            indexing_results.0?;
            indexing_results.1?;
            indexing_results.2?;
            indexing_results.3?;
            indexing_results.4?;
            indexing_results.5?;
            indexing_results.6?;
            indexing_results.7?;
            indexing_results.8?;
            indexing_results.9?;
            indexing_results.10?;

            Ok(())
        }
    }
}

/// Handles the synchronization of a tagged post by updating the graph, indexes, and related counts.
/// # Arguments
/// - `tagger_user_id` - The `PubkyId` of the user tagging the post.
/// - `author_id` - The `PubkyId` of the author of the tagged post.
/// - `post_id` - A `String` representing the unique identifier of the post being tagged.
/// - `tag_id` - A `String` representing the unique identifier of the tag.
/// - `tag_label` - A `String` representing the label of the tag.
/// - `post_uri` - A `String` representing the homeserver URI of the tagged post.
/// - `indexed_at` - A 64-bit integer representing the timestamp when the post was indexed.
///
async fn put_sync_post(
    tagger_user_id: PubkyId,
    author_id: PubkyId,
    post_id: &str,
    tag_id: &str,
    tag_label: &str,
    post_uri: &str,
    indexed_at: i64,
) -> Result<(), EventProcessorError> {
    match TagPost::put_to_graph(
        &tagger_user_id,
        &author_id,
        Some(post_id),
        tag_id,
        tag_label,
        indexed_at,
    )
    .await?
    {
        OperationOutcome::Updated => Ok(()),
        OperationOutcome::MissingDependency => {
            // Ensure that dependencies follow the same format as the RetryManager keys
            let dependency = vec![format!("{author_id}:posts:{post_id}")];
            if let Ok(referenced_post_uri) = ParsedUri::try_from(post_uri) {
                if let Err(e) = PostDetails::maybe_ingest_author_of_post(&referenced_post_uri).await
                {
                    tracing::error!("Failed to ingest user: {e}");
                }
            }
            Err(EventProcessorError::MissingDependency { dependency })
        }
        OperationOutcome::CreatedOrDeleted => {
            // SAVE TO INDEXES
            let post_key_slice: &[&str] = &[&author_id, post_id];
            let tag_label_slice = &[tag_label.to_string()];

            let indexing_results = nexus_common::traced_join!(
                tracing::info_span!("index.write", phase = "tag_post");
                // Update user counts for tagger
                UserCounts::increment(&tagger_user_id, "tagged", None),
                // Increment in one the post tags
                PostCounts::increment_index_field(post_key_slice, "tags", None),
                async {
                    // Increase unique_tags if the tag does not exist already
                    // NOTE: To update that field, it cannot exist in TagPost SORTED SET the tag. Thats why it has to be executed
                    // before TagPost operation
                    PostCounts::increment_index_field(
                        post_key_slice,
                        "unique_tags",
                        Some(tag_label),
                    )
                    .await?;
                    // Increment the label count to post
                    TagPost::update_index_score(
                        &author_id,
                        Some(post_id),
                        tag_label,
                        ScoreAction::Increment(1.0),
                    )
                    .await?;
                    Ok::<(), EventProcessorError>(())
                },
                // Add user tag in post
                TagPost::add_tagger_to_index(&author_id, Some(post_id), &tagger_user_id, tag_label),
                // Add post to label total engagement
                PostsByTagSearch::update_index_score(&author_id, post_id, tag_label, ScoreAction::Increment(1.0)),
                async {
                    // Post replies cannot be included in the total engagement index once they have been tagged
                    if !post_relationships_is_reply(&author_id, post_id).await? {
                        // Increment in one post global engagement
                        PostStream::update_index_score(
                            &author_id,
                            post_id,
                            ScoreAction::Increment(1.0),
                        )
                        .await?;
                    }
                    Ok::<(), EventProcessorError>(())
                },
                // Add post to global label timeline
                PostsByTagSearch::put_to_index(&author_id, post_id, tag_label),
                // Save new notification
                Notification::new_post_tag(&tagger_user_id, &author_id, tag_label, post_uri),
                // Add tag to search index
                TagSearch::put_to_index(tag_label_slice)
            );

            indexing_results.0?;
            indexing_results.1?;
            indexing_results.2?;
            indexing_results.3?;
            indexing_results.4?;
            indexing_results.5?;
            indexing_results.6?;
            indexing_results.7?;
            indexing_results.8?;

            Ok(())
        }
    }
}

/// Handles the synchronization of a tagged user by updating the graph, indexes, and related counts.
///
/// # Arguments
/// - `tagger_user_id` - The `PubkyId` of the user tagging the user.
/// - `tagged_user_id` - The `PubkyId` of the user being tagged.
/// - `tag_id` - A `String` representing the unique identifier of the tag.
/// - `tag_label` - A `String` representing the label of the tag.
/// - `indexed_at` - A 64-bit integer representing the timestamp when the user was indexed.
async fn put_sync_user(
    tagger_user_id: PubkyId,
    tagged_user_id: PubkyId,
    tag_id: &str,
    tag_label: &str,
    indexed_at: i64,
) -> Result<(), EventProcessorError> {
    match TagUser::put_to_graph(
        &tagger_user_id,
        &tagged_user_id,
        None,
        tag_id,
        tag_label,
        indexed_at,
    )
    .await?
    {
        OperationOutcome::Updated => Ok(()),
        OperationOutcome::MissingDependency => {
            if let Err(e) = UserDetails::maybe_ingest_user(tagged_user_id.as_str()).await {
                tracing::error!("Failed to ingest user: {e}");
            }

            let key = RetryEvent::generate_index_key_from_uri(&tagged_user_id.to_uri());
            let dependency = vec![key];
            Err(EventProcessorError::MissingDependency { dependency })
        }
        OperationOutcome::CreatedOrDeleted => {
            let tag_label_slice = &[tag_label.to_string()];

            // SAVE TO INDEX
            let indexing_results = nexus_common::traced_join!(
                tracing::info_span!("index.write", phase = "tag_user");
                // Update user counts for the tagged user
                UserCounts::increment(&tagged_user_id, "tags", None),
                // Update user counts for the tagger user
                UserCounts::increment(&tagger_user_id, "tagged", None),
                async {
                    // Increase unique_tags if the tag does not exist already
                    // NOTE: To update that field, it cannot exist in TagUser SORTED SET the tag. Thats why it has to be executed
                    // before TagUser operation
                    UserCounts::increment(&tagged_user_id, "unique_tags", Some(tag_label)).await?;
                    // Add label count to the user profile tag
                    TagUser::update_index_score(&tagged_user_id, None, tag_label, ScoreAction::Increment(1.0)).await?;
                    Ok::<(), EventProcessorError>(())
                },
                // Add tagger to the user taggers list
                TagUser::add_tagger_to_index(&tagged_user_id, None, &tagger_user_id, tag_label),
                // Save new notification
                Notification::new_user_tag(&tagger_user_id, &tagged_user_id, tag_label),
                // Add tag to search index
                TagSearch::put_to_index(tag_label_slice)
            );

            indexing_results.0?;
            indexing_results.1?;
            indexing_results.2?;
            indexing_results.3?;
            indexing_results.4?;
            indexing_results.5?;

            Ok(())
        }
    }
}

#[tracing::instrument(name = "tag.del", skip_all, fields(user_id = %user_id, tag_id = %tag_id))]
pub async fn del(
    user_id: PubkyId,
    tag_id: String,
    app: Option<String>,
) -> Result<(), EventProcessorError> {
    debug!("Deleting tag: {} -> {} (app={:?})", user_id, tag_id, app);

    // Execute the delete query directly (bypasses TagCollection trait to get resource_id field)
    let query = queries::del::delete_tag(&user_id, &tag_id, app.as_deref());
    let maybe_row = fetch_row_from_graph(query).await?;

    let Some(row) = maybe_row else {
        return Err(EventProcessorError::SkipIndexing);
    };

    let tagged_user_id: Option<String> = row.get("user_id").unwrap_or(None);
    let post_id: Option<String> = row.get("post_id").unwrap_or(None);
    let author_id: Option<String> = row.get("author_id").unwrap_or(None);
    let resource_id: Option<String> = row.get("resource_id").unwrap_or(None);
    let label: String = row
        .get("label")
        .map_err(|e| EventProcessorError::generic(format!("Missing label in delete_tag: {e}")))?;
    let app: Option<String> = row.get("app").unwrap_or(None);

    match (tagged_user_id, post_id, author_id, resource_id) {
        // Delete user related indexes
        (Some(tagged_id), None, None, None) => {
            del_sync_user(user_id, &tagged_id, &label).await?;
        }
        // Delete post related indexes
        (None, Some(post_id), Some(author_id), None) => {
            del_sync_post(user_id, &post_id, &author_id, &label).await?;
        }
        // Delete resource related indexes
        (None, None, None, Some(res_id)) => {
            del_sync_resource(user_id, &res_id, &label, app.as_deref()).await?;
        }
        // Handle other unexpected cases
        _ => {
            debug!("DEL-Tag: Unexpected combination of tag details");
        }
    }
    Ok(())
}

async fn del_sync_user(
    tagger_id: PubkyId,
    tagged_id: &str,
    tag_label: &str,
) -> Result<(), EventProcessorError> {
    let indexing_results = nexus_common::traced_join!(
        tracing::info_span!("index.delete", phase = "tag_user");
        // Update user counts in the tagged
        UserCounts::decrement(tagged_id, "tags", None),
        // Update user counts in the tagger
        UserCounts::decrement(&tagger_id, "tagged", None),
        async {
            // Decrement label count to the user profile tag
            TagUser::update_index_score(tagged_id, None, tag_label, ScoreAction::Decrement(1.0)).await?;
            // Decrease unique_tags
            // NOTE: To update that field, we first need to decrement the value in the TagUser SORTED SET associated with that tag
            UserCounts::decrement(tagged_id, "unique_tags", Some(tag_label)).await?;
            Ok::<(), EventProcessorError>(())
        },
        async {
            // Remove tagger to the user taggers list
            TagUser(vec![tagger_id.to_string()])
                .del_from_index(tagged_id, None, tag_label)
                .await?;
            Ok::<(), EventProcessorError>(())
        },
        // Save new notification
        Notification::new_user_untag(&tagger_id, tagged_id, tag_label)
    );

    indexing_results.0?;
    indexing_results.1?;
    indexing_results.2?;
    indexing_results.3?;
    indexing_results.4?;

    Ok(())
}

async fn del_sync_post(
    tagger_id: PubkyId,
    post_id: &str,
    author_id: &str,
    tag_label: &str,
) -> Result<(), EventProcessorError> {
    // SAVE TO INDEXES
    let post_key_slice: &[&str] = &[author_id, post_id];
    let tag_post = TagPost(vec![tagger_id.to_string()]);
    let post_uri = post_uri_builder(author_id.to_string(), post_id.to_string());

    let indexing_results = nexus_common::traced_join!(
        tracing::info_span!("index.delete", phase = "tag_post");
        // Update user counts for tagger
        UserCounts::decrement(&tagger_id, "tagged", None),
        // Decrement in one the post tags
        PostCounts::decrement_index_field(post_key_slice, "tags", None),
        async {
            // Decrement label score in the post
            TagPost::update_index_score(
                author_id,
                Some(post_id),
                tag_label,
                ScoreAction::Decrement(1.0),
            )
            .await?;
            // Decrease unique_tag
            // NOTE: To update that field, we first need to decrement the value in the SORTED SET associated with that tag
            PostCounts::decrement_index_field(post_key_slice, "unique_tags", Some(tag_label))
                .await?;
            Ok::<(), EventProcessorError>(())
        },
        // Decrease post from label total engagement
        PostsByTagSearch::update_index_score(
            author_id,
            post_id,
            tag_label,
            ScoreAction::Decrement(1.0),
        ),
        async {
            // Post replies cannot be included in the total engagement index once the tag have been deleted
            if !post_relationships_is_reply(author_id, post_id).await? {
                // Decrement in one post global engagement
                PostStream::update_index_score(author_id, post_id, ScoreAction::Decrement(1.0))
                    .await?;
            }
            Ok::<(), EventProcessorError>(())
        },
        async {
            // Delete the tagger from the tag list
            tag_post
                .del_from_index(author_id, Some(post_id), tag_label)
                .await?;
            // NOTE: The tag search index, depends on the post taggers collection to delete
            // Delete post from global label timeline
            PostsByTagSearch::del_from_index(author_id, post_id, tag_label).await?;

            let posts_by_tag =
                PostsByTagSearch::get_by_label(tag_label, None, Pagination::default()).await?;
            let posts_by_tag_found = posts_by_tag.is_some_and(|x| !x.is_empty());
            if !posts_by_tag_found {
                // If we just removed the last post using this tag, remove tag from autocomplete suggestion list
                TagSearch::del_from_index(tag_label).await?;
            }

            Ok::<(), EventProcessorError>(())
        },
        // Save new notification
        Notification::new_post_untag(&tagger_id, author_id, tag_label, &post_uri)
    );

    indexing_results.0?;
    indexing_results.1?;
    indexing_results.2?;
    indexing_results.3?;
    indexing_results.4?;
    indexing_results.5?;
    indexing_results.6?;

    Ok(())
}

/// Cleans up Redis indexes when a Resource tag is deleted.
/// Orphaned Resource node cleanup is handled by the delete_tag Cypher query.
/// Timeline entries are only removed when taggers count reaches zero.
async fn del_sync_resource(
    tagger_id: PubkyId,
    resource_id: &str,
    tag_label: &str,
    app: Option<&str>,
) -> Result<(), EventProcessorError> {
    // Step 1: Decrement scores and remove tagger from sets
    let score_results = tokio::join!(
        TagResource::update_index_score(resource_id, None, tag_label, ScoreAction::Decrement(1.0)),
        async {
            TagResource(vec![tagger_id.to_string()])
                .del_from_index(resource_id, None, tag_label)
                .await?;
            Ok::<(), EventProcessorError>(())
        },
        ResourceStream::update_global_taggers_count(resource_id, ScoreAction::Decrement(1.0)),
        ResourceStream::update_tag_taggers_count(
            tag_label,
            resource_id,
            ScoreAction::Decrement(1.0),
        ),
        async {
            if let Some(a) = app {
                let (r1, r2) = tokio::join!(
                    ResourceStream::update_app_taggers_count(
                        a,
                        resource_id,
                        ScoreAction::Decrement(1.0),
                    ),
                    ResourceStream::update_app_tag_taggers_count(
                        a,
                        tag_label,
                        resource_id,
                        ScoreAction::Decrement(1.0),
                    ),
                );
                r1?;
                r2?;
            }
            Ok::<(), nexus_common::db::kv::RedisError>(())
        }
    );

    score_results.0?;
    score_results.1?;
    score_results.2?;
    score_results.3?;
    score_results.4?;

    // Step 2: Check remaining scores and remove from timelines only when zero.
    remove_timeline_if_empty(
        &["Resources", "Global", "TaggersCount"],
        resource_id,
        ResourceStream::del_from_global_timeline(resource_id),
    )
    .await?;

    remove_timeline_if_empty(
        &["Resources", "Tag", tag_label, "TaggersCount"],
        resource_id,
        ResourceStream::del_from_tag_timeline(tag_label, resource_id),
    )
    .await?;

    if let Some(a) = app {
        remove_timeline_if_empty(
            &["Resources", "App", a, "TaggersCount"],
            resource_id,
            ResourceStream::del_from_app_timeline(a, resource_id),
        )
        .await?;

        remove_timeline_if_empty(
            &["Resources", "App", a, "Tag", tag_label, "TaggersCount"],
            resource_id,
            ResourceStream::del_from_app_tag_timeline(a, tag_label, resource_id),
        )
        .await?;
    }

    Ok(())
}

/// Checks if a resource's score in a taggers-count sorted set is zero or absent,
/// and if so, removes it from the corresponding timeline.
async fn remove_timeline_if_empty(
    count_key_parts: &[&str],
    resource_id: &str,
    delete_fn: impl std::future::Future<Output = RedisResult<()>>,
) -> Result<(), EventProcessorError> {
    let score =
        ResourceStream::check_sorted_set_member(None, count_key_parts, &[resource_id]).await?;
    if score.is_none_or(|s| s <= 0) {
        delete_fn.await?;
    }
    Ok(())
}
