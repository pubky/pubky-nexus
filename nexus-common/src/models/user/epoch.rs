use serde::de::{value, IntoDeserializer};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use utoipa::ToSchema;

use crate::db::kv::{RedisError, RedisResult};
use crate::db::{
    fetch_all_rows_from_graph, fetch_key_from_graph, fetch_row_from_graph, queries, RedisOps,
};
use crate::models::error::ModelResult;

/// A user's indexing epoch: Nexus indexing state, not app data. Users are born `V0`.
#[derive(Serialize, Deserialize, ToSchema, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Epoch {
    #[default]
    V0,
    V1,
}

impl Epoch {
    /// The plain string form (`"v0"`/`"v1"`), the same one serde writes to the
    /// graph property and the JSON field.
    pub fn as_str(self) -> &'static str {
        match self {
            Epoch::V0 => "v0",
            Epoch::V1 => "v1",
        }
    }
}

impl FromStr for Epoch {
    type Err = value::Error;

    /// Parses the plain string form through serde, so `"V1"` is rejected exactly
    /// as it is in the graph and the JSON.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::deserialize(s.into_deserializer())
    }
}

/// The whole HASH is the model's prefix, `User:Epoch`, so no key parts follow it.
const NO_KEY_PARTS: [&str; 0] = [];

/// Users read from the graph per [`UserEpoch::reindex`] page.
const REINDEX_PAGE_SIZE: usize = 1_000;

/// The `User:Epoch` HASH: field = user id, value = the user's [`Epoch`] in
/// its plain string form, so the watcher reads an epoch with one `HGET`.
///
/// A derived index of the `:User.epoch` graph property: every value written here
/// comes from the graph, and [`Self::reindex`] rebuilds it. It is not durable
/// state of its own, unlike the cursors and the retry queue.
///
/// A marker type that owns the Redis reads and writes; it holds no data.
/// `Serialize`/`Deserialize` are only present to satisfy the [`RedisOps`] trait bounds.
#[derive(Serialize, Deserialize)]
pub struct UserEpoch;

#[async_trait::async_trait]
impl RedisOps for UserEpoch {}

impl UserEpoch {
    /// Retrieves a user's epoch, first trying the index, then the graph if not
    /// found; `None` when there is no `:User` node, in which case nothing is written.
    ///
    /// On a miss the epoch read from the graph is indexed. That read may write the
    /// graph: see [`Self::get_from_graph`].
    pub async fn get_by_id(user_id: &str) -> ModelResult<Option<Epoch>> {
        if let Some(epoch) = Self::get_from_index(user_id).await? {
            return Ok(Some(epoch));
        }

        let Some(epoch) = Self::get_from_graph(user_id).await? else {
            return Ok(None);
        };
        Self::put_to_index(user_id, epoch).await?;
        Ok(Some(epoch))
    }

    /// Reads a user's epoch from the index, or `None` when it has no field for them.
    ///
    /// A stored value that is not a known epoch is an error, never a default.
    pub async fn get_from_index(user_id: &str) -> RedisResult<Option<Epoch>> {
        Self::try_from_index_hash(&NO_KEY_PARTS, user_id)
            .await?
            .map(|raw| {
                raw.parse()
                    .map_err(|e| RedisError::DeserializationFailed(Box::new(e)))
            })
            .transpose()
    }

    /// Reads a user's stored epoch from the graph, or `None` when there is no
    /// `:User` node.
    ///
    /// Unlike other `get_from_graph` reads, it writes: a node without an epoch is
    /// first set to `v0`, so the value returned is always the stored one.
    async fn get_from_graph(user_id: &str) -> ModelResult<Option<Epoch>> {
        let Some(row) = fetch_row_from_graph(queries::get::get_user_epoch(user_id)).await? else {
            return Ok(None);
        };
        if let Some(epoch) = row.get::<Option<Epoch>>("epoch")? {
            return Ok(Some(epoch));
        }

        // `None` when the node was deleted since it was read
        let query = queries::put::init_user_epoch(user_id);
        Ok(fetch_key_from_graph(query, "epoch").await?)
    }

    /// Indexes one user's epoch.
    pub async fn put_to_index(user_id: &str, epoch: Epoch) -> RedisResult<()> {
        Self::put_multiple_to_index([(user_id, epoch)]).await
    }

    /// Indexes several users' epochs in one command.
    pub async fn put_multiple_to_index<'a>(
        entries: impl IntoIterator<Item = (&'a str, Epoch)>,
    ) -> RedisResult<()> {
        let entries: Vec<(&str, &str)> = entries
            .into_iter()
            .map(|(user_id, epoch)| (user_id, epoch.as_str()))
            .collect();
        Self::put_index_hash(&NO_KEY_PARTS, &entries).await
    }

    /// Removes a user's field from the index.
    pub async fn del_from_index(user_id: &str) -> RedisResult<()> {
        Self::remove_from_index_hash(&NO_KEY_PARTS, &[user_id]).await
    }

    /// Fills the index from the graph for every `:User`, one keyset page at a
    /// time; a node without an `epoch` is indexed as `v0`.
    ///
    /// Only adds and overwrites fields: after a FLUSH that leaves exactly one
    /// field per `:User`.
    pub async fn reindex() -> ModelResult<()> {
        Self::reindex_in_pages(REINDEX_PAGE_SIZE).await
    }

    async fn reindex_in_pages(page_size: usize) -> ModelResult<()> {
        let mut after_id = String::new();
        loop {
            let query = queries::get::get_user_epochs_page(&after_id, page_size);
            let mut page: Vec<(String, Epoch)> = Vec::with_capacity(page_size);
            for row in fetch_all_rows_from_graph(query).await? {
                page.push((row.get("id")?, row.get("epoch")?));
            }
            let Some((last_id, _)) = page.last() else {
                return Ok(());
            };
            after_id = last_id.clone();

            Self::put_multiple_to_index(page.iter().map(|(id, epoch)| (id.as_str(), *epoch)))
                .await?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Epoch, UserEpoch};
    use crate::db::graph::Query;
    use crate::db::{fetch_all_rows_from_graph, get_redis_conn};
    use crate::types::DynError;
    use crate::{StackConfig, StackManager};
    use deadpool_redis::redis::AsyncCommands;
    use pubky::Keypair;
    use pubky_app_specs::PubkyId;

    #[test]
    fn plain_string_form_matches_serde() {
        for (epoch, s) in [(Epoch::V0, "v0"), (Epoch::V1, "v1")] {
            assert_eq!(epoch.as_str(), s);
            assert_eq!(serde_json::to_value(epoch).unwrap(), s);
            assert_eq!(serde_json::from_value::<Epoch>(s.into()).unwrap(), epoch);
            assert_eq!(s.parse::<Epoch>().unwrap(), epoch);
        }
        assert_eq!(Epoch::default(), Epoch::V0);
    }

    #[test]
    fn rejects_unknown_forms() {
        for raw in ["V1", "v2", ""] {
            assert!(raw.parse::<Epoch>().is_err(), "{raw:?} should not parse");
        }
    }

    const KEY: &str = "User:Epoch";

    fn random_user_id() -> String {
        PubkyId::from(Keypair::random().public_key()).to_string()
    }

    async fn raw_field(user_id: &str) -> Result<Option<String>, DynError> {
        let mut conn = get_redis_conn().await?;
        Ok(conn.hget(KEY, user_id).await?)
    }

    #[tokio_shared_rt::test(shared)]
    async fn put_get_delete_round_trip() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let user_id = random_user_id();

        assert_eq!(UserEpoch::get_from_index(&user_id).await?, None);

        UserEpoch::put_to_index(&user_id, Epoch::V1).await?;
        assert_eq!(UserEpoch::get_from_index(&user_id).await?, Some(Epoch::V1));
        assert_eq!(raw_field(&user_id).await?.as_deref(), Some("v1"));

        UserEpoch::del_from_index(&user_id).await?;
        assert_eq!(UserEpoch::get_from_index(&user_id).await?, None);

        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn put_multiple_writes_every_field() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let (a, b) = (random_user_id(), random_user_id());

        UserEpoch::put_multiple_to_index([(a.as_str(), Epoch::V0), (b.as_str(), Epoch::V1)])
            .await?;
        assert_eq!(raw_field(&a).await?.as_deref(), Some("v0"));
        assert_eq!(raw_field(&b).await?.as_deref(), Some("v1"));

        UserEpoch::put_multiple_to_index([]).await?;
        UserEpoch::del_from_index(&a).await?;
        UserEpoch::del_from_index(&b).await?;

        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn get_rejects_an_unknown_value() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let user_id = random_user_id();

        let mut conn = get_redis_conn().await?;
        let _: () = conn.hset(KEY, &user_id, "V1").await?;
        assert!(UserEpoch::get_from_index(&user_id).await.is_err());

        UserEpoch::del_from_index(&user_id).await?;
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn reindex_pages_through_every_user() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;

        let query = Query::new(
            "epoch_test_user_epochs",
            "MATCH (u:User) RETURN u.id AS id, coalesce(u.epoch, 'v0') AS epoch",
        );
        let mut users: Vec<(String, String)> = Vec::new();
        for row in fetch_all_rows_from_graph(query).await? {
            users.push((row.get("id")?, row.get("epoch")?));
        }
        assert!(users.len() > 3, "the mock graph should span several pages");

        // Start from no fields, so only this rebuild can satisfy the assertions.
        // Nothing but these tests reads User:Epoch yet.
        for (id, _) in &users {
            UserEpoch::del_from_index(id).await?;
        }

        // A page size far below the user count drives the keyset across many pages.
        UserEpoch::reindex_in_pages(3).await?;

        for (id, epoch) in &users {
            assert_eq!(
                raw_field(id).await?.as_deref(),
                Some(epoch.as_str()),
                "{id}"
            );
        }
        Ok(())
    }
}
