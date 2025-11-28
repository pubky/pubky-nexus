use crate::db::kv::ScoreAction;
use crate::db::{execute_graph_operation, queries, OperationOutcome, RedisOps};
use crate::models::link::ExternalLinkDetails;
use crate::models::tag::traits::{TagCollection, TaggersCollection};
use crate::models::tag::TagDetails;
use crate::types::DynError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub const EXTERNAL_TAGS_KEY_PARTS: [&str; 2] = ["ExternalLink", "Tag"];

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema, Default)]
pub struct TagExternal(pub Vec<String>);

impl AsRef<[String]> for TagExternal {
    fn as_ref(&self) -> &[String] {
        &self.0
    }
}

#[async_trait]
impl RedisOps for TagExternal {
    async fn prefix() -> String {
        String::from("ExternalLink:Taggers")
    }
}

#[async_trait]
impl TagCollection for TagExternal {
    fn get_tag_prefix<'a>() -> [&'a str; 2] {
        EXTERNAL_TAGS_KEY_PARTS
    }

    fn create_sorted_set_key_parts<'a>(
        user_id: &'a str,
        _extra_param: Option<&'a str>,
        _is_cache: bool,
    ) -> Vec<&'a str> {
        [&Self::get_tag_prefix()[..], &[user_id]].concat()
    }

    fn create_set_common_key<'a>(
        user_id: &'a str,
        _extra_param: Option<&'a str>,
        _is_cache: bool,
    ) -> Vec<&'a str> {
        vec![user_id]
    }

    fn create_label_index(
        user_id: &str,
        _extra_param: Option<&str>,
        label: &String,
        _is_cache: bool,
    ) -> String {
        format!("{user_id}:{label}")
    }

    fn read_graph_query(user_id: &str, _extra_param: Option<&str>) -> neo4rs::Query {
        queries::get::external_link_tags(user_id)
    }

    async fn update_index_score(
        author_id: &str,
        _extra_param: Option<&str>,
        label: &str,
        score_action: ScoreAction,
    ) -> Result<(), DynError> {
        let key: Vec<&str> = [&Self::get_tag_prefix()[..], &[author_id]].concat();
        Self::put_score_index_sorted_set(&key, &[label], score_action).await
    }

    async fn add_tagger_to_index(
        author_id: &str,
        _extra_param: Option<&str>,
        tagger_user_id: &str,
        tag_label: &str,
    ) -> Result<(), DynError> {
        let key = vec![author_id, tag_label];
        Self::put_index_set(&key, &[tagger_user_id], None, None).await
    }

    async fn put_to_graph(
        tagger_user_id: &str,
        tagged_user_id: &str,
        _extra_param: Option<&str>,
        tag_id: &str,
        label: &str,
        indexed_at: i64,
    ) -> Result<OperationOutcome, DynError> {
        let details =
            ExternalLinkDetails::get(tagged_user_id)
                .await?
                .ok_or_else(|| -> DynError {
                    format!("Missing cached external link details for {tagged_user_id}").into()
                })?;

        let query = queries::put::create_external_link_tag(
            tagger_user_id,
            &details,
            tag_id,
            label,
            indexed_at,
        );

        execute_graph_operation(query).await
    }
}

impl TaggersCollection for TagExternal {
    fn create_label_index<'a>(
        user_id: &'a str,
        _extra_param: Option<&'a str>,
        label: &'a str,
        _is_cache: bool,
    ) -> Vec<&'a str> {
        vec![user_id, label]
    }
}

impl TagExternal {
    pub async fn get_by_id(
        link_id: &str,
        skip_tags: Option<usize>,
        limit_tags: Option<usize>,
        limit_taggers: Option<usize>,
        viewer_id: Option<&str>,
    ) -> Result<Option<Vec<TagDetails>>, DynError> {
        <Self as TagCollection>::get_by_id(
            link_id,
            None,
            skip_tags,
            limit_tags,
            limit_taggers,
            viewer_id,
            None,
        )
        .await
    }

    pub async fn get_taggers_by_id(
        link_id: &str,
        label: &str,
        pagination: crate::types::Pagination,
        viewer_id: Option<&str>,
    ) -> Result<Option<(crate::models::tag::Taggers, bool)>, DynError> {
        <Self as TaggersCollection>::get_tagger_by_id(
            link_id, None, label, pagination, viewer_id, None,
        )
        .await
    }
}
