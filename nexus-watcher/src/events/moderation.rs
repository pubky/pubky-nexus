use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use crate::errors::EventProcessorError;
use crate::events::handlers;
use deadpool_redis::redis::{self, Script};
use nexus_common::db::get_redis_conn;
use nexus_common::db::kv::RedisError;
use nexus_common::models::takedown::record_takedown;
use nexus_common::models::user::UserIngestor;
use nexus_common::WatcherConfig;
use pubky_app_specs::{ParsedUri, PubkyAppTag, PubkyId, Resource};
use tracing::info;

pub struct Moderation {
    /// Moderator trusted user id
    pub id: PubkyId,
    /// Tags to be moderated (tagged content is deleted)
    pub tags: Vec<String>,
}

impl Moderation {
    pub fn from_config(config: &WatcherConfig) -> Arc<Self> {
        Arc::new(Self {
            id: config.moderation_id.clone(),
            tags: config.moderated_tags.clone(),
        })
    }

    /// Check if a tag should trigger deletion of the tagged content.
    ///
    /// Returns `true` if the tag was applied by the moderator and matches
    /// a moderated tag label.
    pub fn should_delete(&self, tag: &PubkyAppTag, tagger_id: &PubkyId) -> bool {
        tagger_id == &self.id && self.tags.contains(&tag.label)
    }

    /// Apply moderation to the tagged resource.
    ///
    /// Parses the embedded URI in the moderator tag and removes the corresponding
    /// resource (tag, user, or file). Posts are hard-deleted when edge-free or
    /// tombstoned when engagement edges remain.
    #[tracing::instrument(name = "moderation.apply", skip_all)]
    pub async fn apply_moderation(
        &self,
        moderator_tag: PubkyAppTag,
        files_path: &Path,
        ingestor: &UserIngestor,
    ) -> Result<(), EventProcessorError> {
        let lahel = moderator_tag.label;
        let moderated_uri = &moderator_tag.uri;

        // A profile is not remembered: its address is the user's, and a moderated user may
        // come back by writing a new profile.
        if !handlers::tag::is_tag_storage_uri(moderated_uri) {
            let parsed_uri = ParsedUri::try_from(moderated_uri.as_str())
                .map_err(EventProcessorError::generic)?;
            match parsed_uri.resource {
                Resource::User => {
                    let user_id = parsed_uri.user_id;
                    info!("Moderation tag '{lahel}' detected. Deleting user profile {user_id}");
                    return handlers::user::del(user_id).await;
                }
                Resource::Post(_) | Resource::Tag(_) | Resource::File(_) => {}
                _ => return Ok(()),
            }
        }

        // Posts, tags and files are remembered before they are deleted, so they stay down
        // even if their own event is read only later, or again, or is being written right
        // now by another loop (see `handle_put_event`): each has an address of its own.
        info!("Moderation tag '{lahel}' detected. Taking down {moderated_uri}");
        record_takedown(moderated_uri).await?;
        Self::remove_taken_down(moderated_uri, files_path, ingestor).await
    }

    /// Deletes the post, tag or file at `uri`, which the moderator took down. Posts are
    /// hard-deleted when edge-free or tombstoned when engagement edges remain, as for a
    /// user's own DEL event.
    ///
    /// The moderator's tag and the check after a write (`remove_if_taken_down`) can both
    /// reach here for the same address at once, from different loops. The deletes read
    /// what is indexed and then subtract counts in separate steps, so two at once would
    /// subtract twice: one runs at a time per address, and the second then finds nothing.
    pub(crate) async fn remove_taken_down(
        uri: &str,
        files_path: &Path,
        ingestor: &UserIngestor,
    ) -> Result<(), EventProcessorError> {
        let lock = RemovalLock::acquire(uri).await?;
        let removed = Self::delete_taken_down(uri, files_path, ingestor).await;
        let released = lock.release().await;
        removed?;
        released
    }

    async fn delete_taken_down(
        uri: &str,
        files_path: &Path,
        ingestor: &UserIngestor,
    ) -> Result<(), EventProcessorError> {
        // ParsedUri does not handle app-specific tag storage paths (Universal Tags), so they
        // must be intercepted first.
        if handlers::tag::is_tag_storage_uri(uri) {
            return handlers::tag::del(uri).await;
        }
        let parsed_uri = ParsedUri::try_from(uri).map_err(EventProcessorError::generic)?;
        let user_id = parsed_uri.user_id;
        match parsed_uri.resource {
            Resource::Post(post_id) => handlers::post::del(user_id, post_id, ingestor).await,
            Resource::Tag(_) => handlers::tag::del(uri).await,
            Resource::File(file_id) => handlers::file::del(&user_id, file_id, files_path).await,
            _ => Ok(()),
        }
    }
}

/// How long a removal may hold its address before the lock expires, should its holder
/// stop without releasing it.
const REMOVAL_LOCK_TTL_MS: u64 = 60_000;
/// How often a second removal of the same address checks whether it may go.
const REMOVAL_LOCK_POLL: Duration = Duration::from_millis(50);

/// Releases the lock only if it still holds this holder's token: an expired lock taken
/// over by another removal is left alone.
static RELEASE_SCRIPT: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        "if redis.call('get', KEYS[1]) == ARGV[1] then
             return redis.call('del', KEYS[1])
         end
         return 0",
    )
});

/// One removal of an address at a time, across the watcher's loops.
struct RemovalLock {
    key: String,
    token: String,
}

impl RemovalLock {
    /// Waits until no other removal holds `uri`, then holds it.
    async fn acquire(uri: &str) -> Result<Self, EventProcessorError> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let key = format!("Moderation:Removing:{uri}");
        let token = format!(
            "{}:{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let attempts = REMOVAL_LOCK_TTL_MS / REMOVAL_LOCK_POLL.as_millis() as u64;
        for _ in 0..=attempts {
            let mut redis_conn = get_redis_conn().await?;
            let taken: Option<String> = redis::cmd("SET")
                .arg(&key)
                .arg(&token)
                .arg("NX")
                .arg("PX")
                .arg(REMOVAL_LOCK_TTL_MS)
                .query_async(&mut redis_conn)
                .await
                .map_err(RedisError::from)?;
            if taken.is_some() {
                return Ok(Self { key, token });
            }
            tokio::time::sleep(REMOVAL_LOCK_POLL).await;
        }
        Err(EventProcessorError::generic(format!(
            "another removal of {uri} still holds it"
        )))
    }

    async fn release(self) -> Result<(), EventProcessorError> {
        let mut redis_conn = get_redis_conn().await?;
        let _: i64 = RELEASE_SCRIPT
            .key(&self.key)
            .arg(&self.token)
            .invoke_async(&mut redis_conn)
            .await
            .map_err(RedisError::from)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_common::types::DynError;
    use nexus_common::{StackConfig, StackManager};

    /// A second removal of an address waits until the first releases it.
    #[tokio_shared_rt::test(shared)]
    async fn a_second_removal_of_an_address_waits_for_the_first() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let uri = format!(
            "pubky://{}/pub/pubky.app/posts/0034FJVRPG4V0",
            pubky::Keypair::random().public_key().to_z32()
        );

        let first = RemovalLock::acquire(&uri).await?;
        let uri_second = uri.clone();
        let mut second = tokio::spawn(async move { RemovalLock::acquire(&uri_second).await });

        let waited = tokio::time::timeout(Duration::from_millis(300), &mut second).await;
        assert!(waited.is_err(), "the second waits while the first holds it");

        first.release().await?;
        let second = tokio::time::timeout(Duration::from_secs(5), second)
            .await
            .expect("the second goes once the first released")??;
        second.release().await?;
        Ok(())
    }
}
