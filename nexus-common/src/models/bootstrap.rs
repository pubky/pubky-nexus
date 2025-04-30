use std::collections::HashSet;

use crate::db::kv::SortOrder;
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
    pub users: UserStream,
    pub posts: PostStream,
    pub list: BootstrapList,
}

#[derive(Serialize, ToSchema, Deserialize, Default, Debug)]
pub struct BootstrapList {
    pub stream: Vec<String>,
    pub influencers: Vec<String>,
    pub recommended: Vec<String>,
}

impl Bootstrap {
    /// Builds an pubky.app bootstrap summary for the specified `user_id`, fetching posts, replies,
    /// active influencers, and personalized suggestions
    ///
    /// # Parameters
    /// - `user_id: &str`  
    ///   The ID of the user whose “ImAlive” stream is being built
    /// - `view_type: ViewType`  
    ///   Controls whether to fetch replies and include full stream entries (`Full`)
    ///   or only base posts (`Partial`)
    pub async fn get_by_id(user_id: &str, view_type: ViewType) -> Result<Option<Self>, DynError> {
        let mut bootstrap = Self::default();
        let mut user_ids = HashSet::new();

        // Boostrap guard: Early return if the user lookup fails, avoiding unnecessary work
        let Some(_) = UserDetails::get_by_id(user_id).await? else {
            return Ok(None);
        };
        user_ids.insert(user_id.to_string());

        let is_full_view_type = view_type == ViewType::Full;

        let post_stream_by_timeline =
            get_post_stream_timeline(user_id, StreamSource::All, 20).await?;

        let post_replies =
            bootstrap.handle_post_stream(post_stream_by_timeline, &mut user_ids, view_type);

        bootstrap.add_influencers(&mut user_ids).await?;
        bootstrap
            .add_recommended_users(&mut user_ids, user_id)
            .await?;
        // // TODO: Missing hot tags
        // HotTags::get_hot_tags(None, None, &hot_tag_filter).await?;

        if is_full_view_type {
            bootstrap
                .fetch_and_handle_replies(post_replies, &mut user_ids, user_id)
                .await?;
        }

        // Merge all the users related with posts, post replies, influencers and recommended
        bootstrap
            .fetch_and_merge_users(&user_ids, Some(user_id))
            .await?;

        // UserViews has also taggers, fetch the missing users UserViews
        if is_full_view_type {
            let missing_taggers = bootstrap.collect_missing_taggers(&user_ids);
            if !missing_taggers.is_empty() {
                bootstrap
                    .fetch_and_merge_users(&missing_taggers, Some(user_id))
                    .await?;
            }
        }
        Ok(Some(bootstrap))
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
                self.list.stream.push(format!("{author_id}:{post_id}"));
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
    async fn fetch_and_merge_users(
        &mut self,
        user_ids: &HashSet<String>,
        viewer_id: Option<&str>,
    ) -> Result<(), DynError> {
        if user_ids.is_empty() {
            return Ok(());
        }
        let user_ids_vec: Vec<String> = user_ids.iter().cloned().collect();
        // TODO: If the user list is too big, we could do in batches
        // for batch in user_ids.chunks(BATCH_SIZE) { ...
        if let Some(user_stream) =
            UserStream::from_listed_user_ids(&user_ids_vec, viewer_id, None).await?
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
    /// - `viewer_id: &str`  
    ///   The ID of the current viewer
    async fn fetch_and_handle_replies(
        &mut self,
        post_replies: Vec<(String, String)>,
        user_ids: &mut HashSet<String>,
        viewer_id: &str,
    ) -> Result<(), DynError> {
        // TODO: Might consider in the future to do in all the requests in parallel
        // tokio::task::JoinSet or tokio::spawn(async move {...
        for (author_id, post_id) in post_replies {
            let reply_stream = get_post_stream_timeline(
                viewer_id,
                StreamSource::PostReplies { author_id, post_id },
                3,
            )
            .await?;
            self.handle_post_stream(reply_stream, user_ids, ViewType::Partial);
        }
        Ok(())
    }

    /// Fetches today’s active influencers and appends their IDs to both the internal `influencers` list
    /// and the provided `user_ids` set
    ///
    /// # Parameters
    /// - `user_ids: &mut HashSet<String>` A mutable reference to a set of user IDs
    ///
    async fn add_influencers(&mut self, user_ids: &mut HashSet<String>) -> Result<(), DynError> {
        if let Some(influencers) =
            Influencers::get_influencers(None, None, 0, 0, Timeframe::Today, true).await?
        {
            influencers.0.into_iter().for_each(|(id, _)| {
                self.list.influencers.push(id.clone());
                user_ids.insert(id);
            });
        }
        Ok(())
    }

    /// Fetches recommended user IDs for the given `viewer_id` and appends them to both
    /// the internal `active_users` list and the provided `user_ids` set
    ///
    /// # Parameters
    /// - `user_ids: &mut HashSet<String>` A mutable reference to a set of user IDs
    /// - `viewer_id: &str` The ID of the user for whom recommended are being generated
    async fn add_recommended_users(
        &mut self,
        user_ids: &mut HashSet<String>,
        viewer_id: &str,
    ) -> Result<(), DynError> {
        if let Some(recommended_users) = UserStream::get_recommended_ids(viewer_id, None).await? {
            recommended_users.into_iter().for_each(|id| {
                self.list.recommended.push(id.clone());
                user_ids.insert(id);
            });
        }
        Ok(())
    }
}

async fn get_post_stream_timeline(
    viewer_id: &str,
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
        Some(viewer_id.to_string()),
        None,
        None,
    )
    .await?
    .unwrap_or_default())
}
