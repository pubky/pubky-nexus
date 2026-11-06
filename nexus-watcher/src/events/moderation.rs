use std::path::Path;
use std::sync::Arc;

use crate::errors::EventProcessorError;
use crate::events::handlers;
use crate::events::handlers::file::{blob_of, blob_takedown_key};
use crate::events::inputs::{Reference, ReferenceTarget};
use crate::events::route::{route, takedown_key, Routed};
use crate::events::v0_to_v1::files::FileMap;
use nexus_common::media::is_v1_file_id;
use nexus_common::models::takedown::record_takedown;
use nexus_common::models::user::UserIngestor;
use nexus_common::WatcherConfig;
use pubky_social_specs::legacy_v0::validate_crockford_id;
use pubky_social_specs::PubkyId;
use tracing::{info, warn};

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
    /// Returns `true` if the tag was applied by the moderator and its `label`
    /// is a moderated tag label.
    pub fn should_delete(&self, label: &str, tagger_id: &PubkyId) -> bool {
        tagger_id == &self.id && self.tags.iter().any(|tag| tag == label)
    }

    /// Apply moderation to the resource a moderator tag points at.
    ///
    /// `label` is the moderator tag's label and `moderated` the reference it tags. Removes the
    /// resource its key names (post, tag, user or file). Posts are hard-deleted when edge-free
    /// or tombstoned when engagement edges remain. Any other target is a no-op. `tag_uri` is
    /// the moderator tag's own address, kept with the takedown.
    #[tracing::instrument(name = "moderation.apply", skip_all)]
    pub async fn apply_moderation(
        &self,
        label: &str,
        moderated: Reference,
        tag_uri: &str,
        files_path: &Path,
        ingestor: &UserIngestor,
    ) -> Result<(), EventProcessorError> {
        let Reference { uri, target } = moderated;
        // TODO(specs-migration): a `Resource` tag is never taken down. A moderator tag on a
        // foreign resource removes nothing, and a file takedown deletes the file but leaves the
        // `Resource` tags on its address.
        match target {
            // A profile is not remembered: its address is the user's, and a moderated user may
            // come back by writing a new profile.
            ReferenceTarget::User(user_id) => {
                info!("Moderation tag '{label}' detected. Deleting user profile {user_id}");
                handlers::user::del(user_id).await
            }
            // `handle_error` drops a non-retryable error at debug level, so a moderator action
            // that cannot apply is logged here, where an operator will see it.
            ReferenceTarget::Foreign | ReferenceTarget::Other => {
                warn!("Moderation tag '{label}' on {uri} not applied: names nothing Nexus deletes");
                Ok(())
            }
            target @ (ReferenceTarget::Post(_)
            | ReferenceTarget::Tag { .. }
            | ReferenceTarget::File(_)) => {
                // Recorded under the moderated address's `takedown_key`, as its events are
                // looked up: a `social/v1` post version folds into its post's address, the
                // resource the delete below removes; a `pubky.app` or another app's address is
                // rewritten by the v0 reader. A `pubky.app` takedown does not reach the
                // author's `social/v1` versions after the flip: the post comes back at its
                // `social/v1` address, where the moderator tags it again.
                let address = takedown_key(&uri).map_err(EventProcessorError::generic)?;

                // A `pubky.app` file address must carry a v0 file id, as its DEL must: a dotted
                // id would name the owner's `social/v1` folder, which the delete would remove.
                if let ReferenceTarget::File(key) = &target {
                    if matches!(route(&uri), Routed::Legacy(_))
                        && validate_crockford_id(&key.resource_id).is_err()
                    {
                        warn!("Moderation tag '{label}' on {uri} not applied: no v0 file id");
                        return Ok(());
                    }
                }
                // Posts, tags and files are remembered before they are deleted, so they stay
                // down even if their own event is read only later, or again, or is being
                // written right now by another loop (see `handle_put_event`): each has an
                // address of its own.
                info!("Moderation tag '{label}' on {uri} detected. Taking down {address}");
                // A v0 file's blob goes with it, keyed by `blob_takedown_key`, so the takedown
                // holds against the flip's copy whichever comes first. The map is read before
                // anything is recorded: once the address is, another loop's
                // `remove_if_taken_down` may delete the `File` node the map is built from.
                if let ReferenceTarget::File(key) = &target {
                    if !is_v1_file_id(&key.resource_id) {
                        let files = FileMap::load(&key.owner_id).await?;
                        if let Some(hash) = files.hash_of(&key.resource_id) {
                            let blob_key = blob_takedown_key(key.owner_id.as_ref(), hash);
                            info!("Taking down {blob_key}, the bytes of {address}");
                            record_takedown(&blob_key, tag_uri).await?;
                            handlers::file::del_blob(&key.owner_id, hash, files_path).await?;
                        }
                    }
                }
                record_takedown(&address, tag_uri).await?;
                Self::remove_taken_down(target, files_path, ingestor).await
            }
        }
    }

    /// Deletes the post, tag or file `target` names, which the moderator took down. Posts are
    /// hard-deleted when edge-free or tombstoned when engagement edges remain, as for a
    /// user's own DEL event. A `social/v1` file is removed at every address of its bytes
    /// ([`blob_takedown_key`]). Deleting what isn't indexed reports
    /// [`EventProcessorError::SkipIndexing`], as a user's DEL does. A user or any other
    /// target is never taken down, so it is a no-op here.
    pub(crate) async fn remove_taken_down(
        target: ReferenceTarget,
        files_path: &Path,
        ingestor: &UserIngestor,
    ) -> Result<(), EventProcessorError> {
        match target {
            ReferenceTarget::Post(key) => handlers::post::del(key, ingestor).await,
            ReferenceTarget::Tag { key } => handlers::tag::del(key).await,
            ReferenceTarget::File(key) if is_v1_file_id(&key.resource_id) => {
                handlers::file::del_blob(&key.owner_id, blob_of(&key.resource_id), files_path).await
            }
            ReferenceTarget::File(key) => handlers::file::del(key, files_path).await,
            ReferenceTarget::User(_) | ReferenceTarget::Foreign | ReferenceTarget::Other => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::route::resolve_reference;

    const USER_ID: &str = "8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo";

    /// A moderator tag on a target Nexus never deletes is a no-op, so the warn line here is
    /// the only trace an unapplied takedown leaves on the live path.
    #[tokio::test]
    #[tracing_test::traced_test]
    async fn apply_moderation_warns_on_a_target_it_never_deletes() {
        let label = "label_to_moderate".to_string();
        let moderation = Moderation {
            id: PubkyId::try_from(USER_ID).unwrap(),
            tags: vec![label.clone()],
        };
        let tag_uri = format!("pubky://{USER_ID}/pub/pubky.app/tags/ABCDEFGHJK");

        for address in [
            "https://example.com/album/123".to_string(),
            format!("pubky://{USER_ID}/pub/pubky.app/follows/{USER_ID}"),
        ] {
            let result = moderation
                .apply_moderation(
                    &label,
                    resolve_reference(&address),
                    &tag_uri,
                    Path::new("/nonexistent"),
                    &nexus_common::utils::test_utils::default_ingestor_tests(),
                )
                .await;

            assert!(result.is_ok(), "{address}: unexpected result {result:?}");
        }
        assert!(logs_contain("not applied"));
    }

    /// A moderator tag on a `pubky.app` file address with a dotted id names no v0 file: it is
    /// not applied, so the owner's `social/v1` folder of that name is left alone, and nothing is
    /// recorded, as the DEL line refuses the same id.
    #[tokio::test]
    #[tracing_test::traced_test]
    async fn apply_moderation_refuses_a_dotted_id_at_a_v0_file_address() {
        let label = "label_to_moderate".to_string();
        let moderation = Moderation {
            id: PubkyId::try_from(USER_ID).unwrap(),
            tags: vec![label.clone()],
        };
        let files = tempfile::TempDir::new().unwrap();
        let v1_id = "8Z8CWH8NVYQY39ZEBFGKQWWEKG.png";
        let v1_folder = files
            .path()
            .join(nexus_common::media::file_folder(USER_ID, v1_id));
        std::fs::create_dir_all(&v1_folder).unwrap();
        std::fs::write(v1_folder.join("main"), b"v1").unwrap();
        let address = format!("pubky://{USER_ID}/pub/pubky.app/files/{v1_id}");

        let result = moderation
            .apply_moderation(
                &label,
                resolve_reference(&address),
                &format!("pubky://{USER_ID}/pub/pubky.app/tags/ABCDEFGHJK"),
                files.path(),
                &nexus_common::utils::test_utils::default_ingestor_tests(),
            )
            .await;

        assert!(result.is_ok(), "{result:?}");
        assert!(v1_folder.join("main").exists(), "the v1 folder must stay");
        assert!(logs_contain("no v0 file id"));
    }

    /// A moderator's takedown of a `social/v1` file removes the bytes under every extension,
    /// `H.png` and `H.bin` alike, where a user's own DEL would remove the one folder its address
    /// names; another blob's folder and the owner's legacy folder stay. No graph is touched.
    #[tokio::test]
    async fn remove_taken_down_removes_every_extension_of_a_v1_file() {
        let files = tempfile::TempDir::new().unwrap();
        let owner = PubkyId::try_from(USER_ID).unwrap();
        let hash = "8Z8CWH8NVYQY39ZEBFGKQWWEKG";
        let folders: Vec<std::path::PathBuf> = [
            format!("{hash}.png"),
            format!("{hash}.bin"),
            "9Z8CWH8NVYQY39ZEBFGKQWWEKG.png".to_string(),
            "0032SSN7Q4EVG".to_string(),
        ]
        .iter()
        .map(|id| {
            files
                .path()
                .join(nexus_common::media::file_folder(USER_ID, id))
        })
        .collect();
        for folder in &folders {
            std::fs::create_dir_all(folder).unwrap();
            std::fs::write(folder.join("main"), b"bytes").unwrap();
        }

        Moderation::remove_taken_down(
            ReferenceTarget::File(crate::events::inputs::ResourceKey {
                owner_id: owner,
                resource_id: format!("{hash}.png"),
            }),
            files.path(),
            &nexus_common::utils::test_utils::default_ingestor_tests(),
        )
        .await
        .unwrap();

        assert!(!folders[0].exists(), "H.png must go");
        assert!(
            !folders[1].exists(),
            "H.bin, the same bytes, must go with it"
        );
        assert!(folders[2].exists(), "another blob's folder stays");
        assert!(folders[3].exists(), "the owner's legacy folder stays");
    }
}
