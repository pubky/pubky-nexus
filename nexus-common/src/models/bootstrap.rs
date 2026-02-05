use std::collections::HashSet;

use crate::db::kv::SortOrder;
use crate::models::tag::stream::{HotTag, HotTags};
use crate::models::user::Muted;
use crate::types::routes::HotTagsInputDTO;
use crate::types::{DynError, Pagination, StreamSorting, Timeframe};

use crate::models::{
    post::{PostStream, StreamSource},
    tag::TagDetails,
    user::{Influencers, UserStream},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::user::UserDetails;

#[derive(PartialEq, Deserialize)]
pub enum ViewType {
    Full,
    Partial,
}

#[derive(Serialize, ToSchema, Deserialize, Default, Debug)]
pub struct Bootstrap {
    /// The user objects shown to the given user ID
    pub users: UserStream,
    /// The posts objects shown to the given user ID
    pub posts: PostStream,
    /// IDs of objects shown to this user on the home page of the FE
    pub ids: BootstrapIds,
    /// Whether or not this user is already indexed
    pub indexed: bool,
}

/// IDs of objects relevant to the bootstrap payload, for example
/// the lists shown on the right panel of the FE.
///
/// Given as IDs because the full referenced objects might already
/// be trasferred in the streams of this Bootstrap payload.
#[derive(Serialize, ToSchema, Deserialize, Default, Debug)]
pub struct BootstrapIds {
    /// Post stream
    pub stream: Vec<String>,
    pub influencers: Vec<String>,
    /// Recommended users for the given user ID
    pub recommended: Vec<String>,
    pub hot_tags: Vec<HotTag>,
    /// User IDs muted by the given user
    pub muted: Vec<String>,
}

impl Bootstrap {
    /// Builds a pubky.app bootstrap summary for the specified `user_id`, fetching posts, replies,
    /// active influencers, and personalized suggestions.
    ///
    /// Returns a populated response even if the user is not found or not indexed.
    ///
    /// # Parameters
    /// - `user_id: &str`  
    ///   The ID of the user whose “ImAlive” stream is being built
    /// - `view_type: ViewType`  
    ///   Controls whether to fetch replies and include full stream entries (`Full`)
    ///   or only base posts (`Partial`)
    pub async fn get_by_id(user_id: &str, view_type: ViewType) -> Result<Self, DynError> {
        let mut bootstrap = Self::default();
        let mut user_ids = HashSet::new();

        let maybe_viewer_id = UserDetails::get_by_id(user_id).await?.map(|_| {
            user_ids.insert(user_id.to_string());
            bootstrap.indexed = true;
            user_id
        });

        let is_full_view_type = view_type == ViewType::Full;

        let post_stream_by_timeline =
            Self::get_post_stream_timeline(maybe_viewer_id, StreamSource::All, 20).await?;

        let post_replies =
            bootstrap.handle_post_stream(post_stream_by_timeline, &mut user_ids, view_type);

        // Populate the user list
        bootstrap.add_influencers(&mut user_ids).await?;

        // User is not indexed, so cannot recommend users until it is indexed
        if maybe_viewer_id.is_some() {
            bootstrap
                .add_recommended_users(&mut user_ids, user_id)
                .await?;
        }

        bootstrap.add_global_hot_tags(&mut user_ids).await?;

        // Start fetching the replies of the posts
        if is_full_view_type {
            bootstrap
                .get_and_handle_replies(post_replies, &mut user_ids, maybe_viewer_id)
                .await?;
        }

        // Merge all the users related with posts, post replies, influencers and recommended
        bootstrap
            .get_and_merge_users(&user_ids, maybe_viewer_id)
            .await?;

        // UserViews has also taggers, fetch the missing users UserViews
        if is_full_view_type {
            let missing_taggers = bootstrap.collect_missing_taggers(&user_ids);
            bootstrap
                .get_and_merge_users(&missing_taggers, maybe_viewer_id)
                .await?;
        }

        // Return only ids in case of muted
        bootstrap.add_muted(maybe_viewer_id).await?;

        Ok(bootstrap)
    }

    /// Processes a stream of posts, collecting reply references, adding post taggers and populating the post stream
    /// in the response object
    ///
    /// # Parameters
    /// - `post_stream`: The `PostStream` whose contained posts will be processed
    /// - `user_ids`: A mutable set of user IDs; authors and taggers encountered will be inserted
    /// - `view_type`: Indicates whether to operate in `Full` mode (recording stream entries and replies)
    fn handle_post_stream(
        &mut self,
        post_stream: PostStream,
        user_ids: &mut HashSet<String>,
        view_type: ViewType,
    ) -> Vec<(String, String)> {
        let is_full_view_type = view_type == ViewType::Full;
        let mut post_replies = Vec::with_capacity(post_stream.0.len());

        for post_view in post_stream.0.iter() {
            let author_id = post_view.details.author.clone();
            let post_id = post_view.details.id.clone();

            if is_full_view_type && post_view.counts.replies > 0 {
                post_replies.push((author_id.clone(), post_id.clone()))
            }
            // Add the author of the post
            user_ids.insert(author_id.clone());
            // Get all the taggers related with the post
            Self::insert_taggers_id(&post_view.tags, user_ids);
            // Include the post in the stream list
            if is_full_view_type {
                self.ids.stream.push(format!("{author_id}:{post_id}"));
            }
        }
        // After analyse the posts, authors and tags, push the stream
        self.posts.extend(post_stream);
        post_replies
    }

    /// Collects all tagger IDs from the current `users` view that are not yet present
    /// in the given `user_ids` set
    ///
    /// # Parameters
    ///
    /// - `user_ids`: A set of user IDs that have already been fetched or seen
    fn collect_missing_taggers(&self, user_ids: &HashSet<String>) -> HashSet<String> {
        let mut missing_taggers = HashSet::new();
        for user in self.users.0.iter() {
            user.tags
                .iter()
                .flat_map(|tags| tags.taggers.iter())
                .for_each(|tagger| {
                    if !user_ids.contains(tagger) {
                        missing_taggers.insert(tagger.clone());
                    }
                });
        }
        missing_taggers
    }

    /// Appends each tagger’s user ID from the given post tag details into the provided set
    ///
    /// # Parameters
    /// - `tag_details_list: &Vec<TagDetails>`  
    ///   A reference to a vector of `TagDetails`, each containing a list of tagger IDs
    /// - `users_list: &mut HashSet<String>`  
    ///   A mutable reference to a set of user IDs; each tagger ID will be inserted here
    fn insert_taggers_id(tag_details_list: &[TagDetails], users_list: &mut HashSet<String>) {
        for tag_details in tag_details_list.iter() {
            for tagger_pk in tag_details.taggers.iter() {
                users_list.insert(tagger_pk.to_string());
            }
        }
    }

    /// Fetches and appends user views for the given set of `user_ids`
    ///
    /// # Parameters
    /// - `user_ids: HashSet<String>`  
    ///   A set of unique user IDs to fetch views for
    /// - `viewer_id: Option<&str>`  
    ///   Optional context user ID for personalized view generation
    async fn get_and_merge_users(
        &mut self,
        user_ids: &HashSet<String>,
        maybe_viewer_id: Option<&str>,
    ) -> Result<(), DynError> {
        if user_ids.is_empty() {
            return Ok(());
        }
        let user_ids_vec: Vec<String> = user_ids.iter().cloned().collect();
        // TODO: If the user list is too big, we could do in batches
        // for batch in user_ids.chunks(BATCH_SIZE) { ...
        if let Some(user_stream) =
            UserStream::from_listed_user_ids(&user_ids_vec, maybe_viewer_id, None).await?
        {
            self.users.extend(user_stream);
        }
        Ok(())
    }

    /// Fetches up to three replies for each post in `post_replies` and integrates their authors (and any taggers)
    /// into both the internal user list
    ///
    /// # Parameters
    /// - `post_replies: Vec<(String, String)>`  
    ///   A list of `(author_id, post_id)` tuples indicating which post replies to fetch
    /// - `user_ids: &mut HashSet<String>`  
    ///   A mutable reference to a set where each reply’s author ID (and any taggers) will be appended
    /// - `maybe_viewer_id: Option<&str>`  
    ///   The ID of the current viewer
    async fn get_and_handle_replies(
        &mut self,
        post_replies: Vec<(String, String)>,
        user_ids: &mut HashSet<String>,
        maybe_viewer_id: Option<&str>,
    ) -> Result<(), DynError> {
        // TODO: Might consider in the future to do in all the requests in parallel
        // tokio::task::JoinSet or tokio::spawn(async move {...
        for (author_id, post_id) in post_replies {
            let reply_stream = Self::get_post_stream_timeline(
                maybe_viewer_id,
                StreamSource::PostReplies { author_id, post_id },
                3,
            )
            .await?;
            self.handle_post_stream(reply_stream, user_ids, ViewType::Partial);
        }
        Ok(())
    }

    /// Fetches a post stream timeline for the given `source` and `limit`
    ///
    /// # Parameters
    /// - `maybe_viewer_id: Option<&str>`  
    ///   Optional context user ID for personalized view generation
    /// - `source: StreamSource`  
    ///   The source of the post stream
    /// - `limit: usize`  
    ///   The limit of the post stream
    async fn get_post_stream_timeline(
        maybe_viewer_id: Option<&str>,
        source: StreamSource,
        limit: usize,
    ) -> Result<PostStream, DynError> {
        let pagination = Pagination {
            skip: Some(0),
            limit: Some(limit),
            start: None,
            end: None,
        };
        Ok(PostStream::get_posts(
            source,
            pagination,
            SortOrder::default(),
            StreamSorting::Timeline,
            maybe_viewer_id.map(|id| id.to_string()),
            None,
            None,
        )
        .await?
        .unwrap_or_default())
    }

    /// Fetches today’s active influencers and appends their IDs to both the internal `influencers` list
    /// and the provided `user_ids` set
    ///
    /// # Parameters
    /// - `user_ids: &mut HashSet<String>` A mutable reference to a set of user IDs
    async fn add_influencers(&mut self, user_ids: &mut HashSet<String>) -> Result<(), DynError> {
        if let Some(influencers) =
            Influencers::get_influencers(None, None, 0, 0, Timeframe::Today, true).await?
        {
            influencers.0.into_iter().for_each(|(id, _)| {
                self.ids.influencers.push(id.clone());
                user_ids.insert(id);
            });
        }
        Ok(())
    }

    async fn add_muted(&mut self, maybe_viewer_id: Option<&str>) -> Result<(), DynError> {
        if let Some(viewer_id) = maybe_viewer_id {
            if let Ok(Some(muted_ids)) = Muted::get_by_id(viewer_id, None, None).await {
                self.ids.muted = muted_ids.0;
            }
        }
        Ok(())
    }

    /// Fetches recommended user IDs for the given `user_id` and appends them to both
    /// the internal `active_users` list and the provided `user_ids` set
    ///
    /// # Parameters
    /// - `user_ids: &mut HashSet<String>` A mutable reference to a set of user IDs
    /// - `user_id: &str` The ID of the user for whom recommended are being generated
    async fn add_recommended_users(
        &mut self,
        user_ids: &mut HashSet<String>,
        user_id: &str,
    ) -> Result<(), DynError> {
        if let Some(recommended_users) = UserStream::get_recommended_ids(user_id, None).await? {
            recommended_users.into_iter().for_each(|id| {
                self.ids.recommended.push(id.clone());
                user_ids.insert(id);
            });
        }
        Ok(())
    }

    /// Fetches today’s global hot tags and appends their IDs to both
    /// the internal `hot_tags` list and the provided `user_ids` set
    ///
    /// # Parameters
    /// - `user_ids: &mut HashSet<String>` A mutable reference to a set of user IDs
    ///
    async fn add_global_hot_tags(
        &mut self,
        user_ids: &mut HashSet<String>,
    ) -> Result<(), DynError> {
        let hot_tag_filter = HotTagsInputDTO::new(Timeframe::Today, 40, 0, 20, None);
        if let Some(today_hot_tags) = HotTags::get_hot_tags(None, None, &hot_tag_filter).await? {
            today_hot_tags.iter().for_each(|tag| {
                self.ids.hot_tags.push(tag.clone());
                tag.taggers_id.iter().for_each(|tagger| {
                    user_ids.insert(tagger.to_string());
                });
            });
        }
        Ok(())
    }
}
