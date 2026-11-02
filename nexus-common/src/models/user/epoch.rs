use crate::db::graph::Query;
use crate::db::{exec_single_row, fetch_key_from_graph, queries, GraphResult};
use serde::de::{value, IntoDeserializer};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use utoipa::ToSchema;

/// A user's indexing epoch: Nexus indexing state, not app data. Users are born `V0`.
///
/// Stored as `epoch` on the `:User` node (a node without one reads as `V0`) and
/// carried by [`super::UserDetails`] for the API. The watcher reads it from the
/// graph only, with [`Epoch::get_from_graph`]: the cached copy may be stale. It
/// has no index of its own.
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

    /// The epoch stored on `user_id`'s `User` node; `V0` when the node has none, or when
    /// there is no node yet.
    pub async fn get_from_graph(user_id: &str) -> GraphResult<Self> {
        Self::fetch_from_graph(queries::get::get_user_epoch(user_id)).await
    }

    /// Runs `query`, which returns a user's stored epoch as `epoch`; `V0` when it returns none.
    pub(crate) async fn fetch_from_graph(query: Query) -> GraphResult<Self> {
        let epoch = fetch_key_from_graph(query, "epoch").await?;
        Ok(epoch.unwrap_or_default())
    }

    /// Moves `user_id`'s `User` node to `V1`; a missing node stays missing. Nothing writes
    /// `V0` over a stored epoch, so the epoch only moves up.
    pub async fn set_v1_in_graph(user_id: &str) -> GraphResult<()> {
        exec_single_row(queries::put::set_user_epoch_v1(user_id)).await
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

#[cfg(test)]
mod tests {
    use super::Epoch;
    use crate::db::{exec_single_row, queries};
    use crate::models::traits::Collection;
    use crate::models::user::UserDetails;
    use crate::{types::DynError, utils::test_utils::random_pubky_id, StackConfig, StackManager};

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

    /// The graph read gives `V0` for a missing node and a node without an epoch, and the
    /// stored value otherwise.
    #[tokio_shared_rt::test(shared)]
    async fn get_from_graph_reads_the_node() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;

        let user_id = random_pubky_id();
        let id = user_id.to_string();
        assert_eq!(Epoch::get_from_graph(&id).await?, Epoch::V0, "no node");

        UserDetails::from_pubky(user_id).put_to_graph().await?;
        assert_eq!(Epoch::get_from_graph(&id).await?, Epoch::V0, "no epoch");

        Epoch::set_v1_in_graph(&id).await?;
        assert_eq!(Epoch::get_from_graph(&id).await?, Epoch::V1, "stored v1");

        exec_single_row(queries::del::delete_user(&id)).await?;
        Ok(())
    }

    /// The `v1` write leaves a missing node missing.
    #[tokio_shared_rt::test(shared)]
    async fn set_v1_in_graph_creates_no_node() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;

        let id = random_pubky_id().to_string();
        Epoch::set_v1_in_graph(&id).await?;
        assert!(UserDetails::get_from_graph(&[id.as_str()]).await?[0].is_none());
        Ok(())
    }

    /// A profile or tombstone write sets the stored epoch on the struct and keeps it, the
    /// in-memory `V0` notwithstanding.
    #[tokio_shared_rt::test(shared)]
    async fn user_writes_set_and_keep_the_stored_epoch() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;

        let user_id = random_pubky_id();
        let id = user_id.to_string();
        let uri = format!("pubky://{id}/pub/social/v1/profile.json");
        let mut profile = UserDetails::from_pubky(user_id.clone());
        profile.put_profile_to_graph(&uri).await?;
        assert_eq!(profile.epoch, Epoch::V0, "new node");

        Epoch::set_v1_in_graph(&id).await?;
        let mut profile = UserDetails::from_pubky(user_id.clone());
        profile.put_profile_to_graph(&uri).await?;
        assert_eq!(profile.epoch, Epoch::V1, "profile");
        let mut tombstone = UserDetails::tombstone(&user_id);
        tombstone.put_tombstone_to_graph().await?;
        assert_eq!(tombstone.epoch, Epoch::V1, "tombstone");
        assert_eq!(Epoch::get_from_graph(&id).await?, Epoch::V1, "kept");

        exec_single_row(queries::del::delete_user(&id)).await?;
        Ok(())
    }
}
