//! Feed state for generic post versions. No app-owned graph nodes are created.

use super::resource_changes::{
    write_seed_batch, ResourceChange, ResourceChangeKind, ResourceChangeOp, BATCH_SIZE,
};
use crate::db::{get_redis_conn, kv::RedisError};
use crate::models::app_post::{valid_namespace, AppPostUri};
use crate::models::error::{ModelError, ModelResult};
use crate::universal_tag::normalize::{normalize_uri, resource_id};
use deadpool_redis::redis;

pub(super) const POST_VERSIONS_KEY: &str = "Event:ResourceChanges:PostVersions";

impl ResourceChange {
    /// The URI names the logical post; the replay key names its actual stored version.
    pub fn post(path: &AppPostUri, op: ResourceChangeOp) -> ModelResult<Self> {
        if path.version.is_none() || !valid_namespace(&path.namespace) {
            return Err(ModelError::InvalidInput(
                "A post reference requires a stored namespace version".into(),
            ));
        }
        let (uri, _) = normalize_uri(&path.uri()).map_err(ModelError::InvalidInput)?;
        Ok(Self {
            op,
            resource_id: resource_id(&uri),
            uri,
            kind: ResourceChangeKind::Post,
            author: path.author.to_string(),
            label: None,
            object_path: path.version_path(),
            at: chrono::Utc::now().timestamp_millis(),
        })
    }
}

/// The version registry and feed append are one Redis operation. Seeding copies
/// accepted, still-present versions; concurrent writes remain in the live suffix.
pub(super) async fn seed_versions(temp: &str) -> ModelResult<()> {
    let mut conn = get_redis_conn().await?;
    let mut cursor = 0u64;
    loop {
        let (next, records): (u64, Vec<(String, String)>) = redis::cmd("HSCAN")
            .arg(POST_VERSIONS_KEY)
            .arg(cursor)
            .arg("COUNT")
            .arg(BATCH_SIZE)
            .query_async(&mut conn)
            .await
            .map_err(RedisError::from)?;
        let lines: Vec<String> = records
            .into_iter()
            .filter(|(uri, _)| AppPostUri::parse(uri).is_some_and(|path| path.version.is_some()))
            .map(|(_, line)| line)
            .collect();
        if !lines.is_empty() {
            write_seed_batch(temp, &lines).await?;
        }
        cursor = next;
        if cursor == 0 {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{StackConfig, StackManager};

    #[tokio_shared_rt::test(shared)]
    async fn live_version_seed_and_delete_do_not_require_graph_nodes(
    ) -> Result<(), crate::types::DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let author = pubky::Keypair::random().public_key().to_z32();
        let first = AppPostUri::parse(&format!(
            "pubky://{author}/pub/example/v1/posts/0032SSN7Q4EVG/0032SSN7Q4EVG.json"
        ))
        .unwrap();
        let mut second = first.clone();
        second.version = Some("0032SSN7Q4EVJ".into());
        for path in [&first, &second] {
            ResourceChange::post(path, ResourceChangeOp::Put)?
                .store()
                .await?;
        }
        let before_epoch = ResourceChange::get_from_index(0, 0).await?.1;
        ResourceChange::reseed().await?;
        let (page, after_epoch, _) = ResourceChange::get_from_index(0, 1000).await?;
        assert_ne!(before_epoch, after_epoch);
        assert_eq!(
            page.lines
                .iter()
                .filter(|line| line.author == author && line.kind == ResourceChangeKind::Post)
                .count(),
            2
        );
        ResourceChange::post(&first, ResourceChangeOp::Del)?
            .store()
            .await?;
        ResourceChange::post(&first, ResourceChangeOp::Del)?
            .store()
            .await?;
        ResourceChange::reseed().await?;
        let page = ResourceChange::get_from_index(0, 1000).await?.0;
        let own: Vec<_> = page
            .lines
            .iter()
            .filter(|line| line.author == author)
            .collect();
        assert_eq!(own.len(), 1);
        assert_eq!(own[0].uri, first.uri());
        assert_eq!(own[0].object_path, second.version_path());
        ResourceChange::post(&second, ResourceChangeOp::Del)?
            .store()
            .await?;
        let mut logical = first;
        logical.version = None;
        assert!(ResourceChange::post(&logical, ResourceChangeOp::Del).is_err());
        Ok(())
    }
}
