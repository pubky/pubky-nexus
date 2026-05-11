use crate::db::graph::exec::execute_graph_operation;
use crate::db::graph::Query;
use crate::db::kv::{RedisResult, ScoreAction};
use crate::db::{queries, GraphResult, OperationOutcome, RedisOps};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::models::tag::traits::{TagCollection, TaggersCollection};

pub const RESOURCE_TAGS_KEY_PARTS: [&str; 2] = ["Resources", "Tag"];

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema, Default)]
pub struct TagResource(pub Vec<String>);

impl AsRef<[String]> for TagResource {
    fn as_ref(&self) -> &[String] {
        &self.0
    }
}

#[async_trait]
impl RedisOps for TagResource {
    async fn prefix() -> String {
        String::from("Resource:Taggers")
    }
}

#[async_trait]
impl TagCollection for TagResource {
    fn get_tag_prefix<'a>() -> [&'a str; 2] {
        RESOURCE_TAGS_KEY_PARTS
    }

    fn read_graph_query(resource_id: &str, _extra_param: Option<&str>) -> Query {
        queries::get::resource_tags(resource_id)
    }

    /// Override: use RESOURCE_TAGS_KEY_PARTS instead of hardcoded USER/POST keys.
    /// The default trait impl uses USER_TAGS_KEY_PARTS when extra_param is None,
    /// which would write to the wrong Redis key for Resources.
    async fn update_index_score(
        resource_id: &str,
        _extra_param: Option<&str>,
        label: &str,
        score_action: ScoreAction,
    ) -> RedisResult<()> {
        let key: Vec<&str> = [&RESOURCE_TAGS_KEY_PARTS[..], &[resource_id]].concat();
        Self::put_score_index_sorted_set(&key, &[label], score_action).await
    }
}

impl TaggersCollection for TagResource {}

impl TagResource {
    /// Creates or merges a TAGGED relationship between a user and a Resource node.
    /// Unlike Post/User tags which use MATCH (target must exist), this uses MERGE
    /// for the Resource (first tag creates it).
    #[allow(clippy::too_many_arguments)]
    pub async fn put_to_graph_resource(
        tagger_id: &str,
        resource_id: &str,
        uri: &str,
        scheme: &str,
        app: &str,
        tag_id: &str,
        label: &str,
        indexed_at: i64,
    ) -> GraphResult<OperationOutcome> {
        let query = queries::put::create_resource_tag(
            tagger_id,
            resource_id,
            uri,
            scheme,
            app,
            tag_id,
            label,
            indexed_at,
        );
        execute_graph_operation(query).await
    }
}
