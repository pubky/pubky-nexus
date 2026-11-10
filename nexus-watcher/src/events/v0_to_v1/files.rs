//! The file map: what the migrator learned from the user's v0 `File` objects, rebuilt from the
//! `File` nodes Nexus holds for them. Every node of the owner counts, attached or not, because
//! several uploads can share a blob and the lowest `tsid` names the blob's extension.
//!
//! On the migrator's terms, a file's blob `{hash}` comes from `File.src`: the v0 parser reads it,
//! its user must be the file's owner, `resolve_deref` turns it into `files/{hash}`, and the
//! prefix comes off. `resolve_deref` only checks that `src` is a blob, so the owner check is
//! separate. A file whose `src` fails either check names no blob, so a reference to it keeps
//! its address. The extension is `mime_to_ext(content_type)`, `bin` for an unlisted type. The
//! `File` nodes themselves are not rewritten: the retry needs them and a delete can't be undone.

use crate::events::EventProcessorError;
use nexus_common::db::{fetch_all_rows_from_graph, queries, GraphError};
use pubky_social_specs::legacy_v0::ParsedUri;
use pubky_social_specs::{mime_to_ext, resolve_deref, PubkyId};
use std::collections::{BTreeMap, HashMap};

/// One of the owner's blobs, as the flip copies it: the address it migrates under and the
/// files that name it, lowest `tsid` first, so a copy reads the namer's `main` before any other.
#[derive(Debug, PartialEq, Eq)]
pub struct Blob {
    pub hash: String,
    pub ext: String,
    pub tsids: Vec<String>,
}

impl Blob {
    /// The `{hash}.{ext}` the blob migrates under: its `social/v1` file id.
    pub fn file_id(&self) -> String {
        format!("{}.{}", self.hash, self.ext)
    }
}

/// The blob each of a user's files names, and the extension each blob migrates under.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct FileMap {
    /// `tsid → hash`, for the files whose `src` is the owner's blob.
    hashes: HashMap<String, String>,
    /// `hash → (namer, ext)`: the lowest `tsid` that names the blob, and its extension.
    exts: HashMap<String, (String, String)>,
}

impl FileMap {
    /// Builds the map of `owner`'s files from the graph.
    pub async fn load(owner: &PubkyId) -> Result<Self, EventProcessorError> {
        let query = queries::migration::get_user_files_sources(owner.as_ref());
        let rows = fetch_all_rows_from_graph(query).await?;
        let files = rows
            .iter()
            .map(|row| {
                Ok((
                    row.get::<String>("id").map_err(GraphError::from)?,
                    row.get::<String>("src").map_err(GraphError::from)?,
                    row.get::<String>("content_type")
                        .map_err(GraphError::from)?,
                ))
            })
            .collect::<Result<Vec<_>, GraphError>>()?;
        Ok(Self::from_files(owner, files))
    }

    /// Builds the map from `(tsid, src, content_type)` rows, in any order: the namer of a blob
    /// is its lowest `tsid` whichever row comes first.
    pub fn from_files(
        owner: &PubkyId,
        files: impl IntoIterator<Item = (String, String, String)>,
    ) -> Self {
        let mut map = Self::default();
        for (tsid, src, content_type) in files {
            let own = ParsedUri::try_from(src.as_str()).is_ok_and(|p| p.user_id == *owner);
            let Some(hash) = own
                .then(|| resolve_deref(&tsid, &src))
                .flatten()
                .and_then(|key| key.strip_prefix("files/").map(str::to_string))
            else {
                continue;
            };
            if map.exts.get(&hash).is_none_or(|(namer, _)| tsid < *namer) {
                map.exts
                    .insert(hash.clone(), (tsid.clone(), mime_to_ext(&content_type)));
            }
            map.hashes.insert(tsid, hash);
        }
        map
    }

    /// The blob the owner's file `tsid` names, if its `src` was the owner's blob.
    pub fn hash_of(&self, tsid: &str) -> Option<&str> {
        self.hashes.get(tsid).map(String::as_str)
    }

    /// The extension the owner's blob `hash` migrates under: its namer's, `bin` when no file
    /// names it, as in the migrator.
    pub fn ext_of(&self, hash: &str) -> &str {
        self.exts.get(hash).map_or("bin", |(_, ext)| ext.as_str())
    }

    /// Every blob the owner's files name, in hash order, each with its files lowest `tsid`
    /// first: the order the flip copies them in, so a rerun walks the same list.
    pub fn blobs(&self) -> Vec<Blob> {
        let mut by_hash: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for (tsid, hash) in &self.hashes {
            by_hash.entry(hash).or_default().push(tsid);
        }
        by_hash
            .into_iter()
            .map(|(hash, mut tsids)| {
                tsids.sort_unstable();
                Blob {
                    hash: hash.to_string(),
                    ext: self.ext_of(hash).to_string(),
                    tsids: tsids.into_iter().map(str::to_string).collect(),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWNER: &str = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
    const OTHER: &str = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";
    const HASH: &str = "8Z8CWH8NVYQY39ZEBFGKQWWEKG";
    const OTHER_HASH: &str = "2ZK3A1B2C3D4E5F6G7H8J9K0MN";

    fn owner() -> PubkyId {
        PubkyId::try_from(OWNER).unwrap()
    }

    fn blob(user: &str, hash: &str) -> String {
        format!("pubky://{user}/pub/pubky.app/blobs/{hash}")
    }

    fn row(tsid: &str, src: &str, content_type: &str) -> (String, String, String) {
        (tsid.to_string(), src.to_string(), content_type.to_string())
    }

    #[test]
    fn an_own_blob_src_maps_the_file_to_its_hash_and_extension() {
        let map = FileMap::from_files(
            &owner(),
            [row("0032SSN7Q4EVG", &blob(OWNER, HASH), "image/png")],
        );

        assert_eq!(map.hash_of("0032SSN7Q4EVG"), Some(HASH));
        assert_eq!(map.ext_of(HASH), "png");
    }

    #[test]
    fn the_lowest_tsid_names_the_extension_whatever_the_order() {
        let rows = [
            row("0032SSN7Q4EVH", &blob(OWNER, HASH), "text/plain"),
            row("0032SSN7Q4EVG", &blob(OWNER, HASH), "image/png"),
            row("0032SSN7Q4EVJ", &blob(OWNER, HASH), "application/pdf"),
        ];
        let forward = FileMap::from_files(&owner(), rows.clone());
        let backward = FileMap::from_files(&owner(), rows.into_iter().rev());

        assert_eq!(forward, backward);
        assert_eq!(forward.ext_of(HASH), "png");
        for tsid in ["0032SSN7Q4EVG", "0032SSN7Q4EVH", "0032SSN7Q4EVJ"] {
            assert_eq!(forward.hash_of(tsid), Some(HASH));
        }
    }

    #[test]
    fn an_unlisted_content_type_and_an_unnamed_blob_get_bin() {
        let map = FileMap::from_files(
            &owner(),
            [row(
                "0032SSN7Q4EVG",
                &blob(OWNER, HASH),
                "application/octet-stream",
            )],
        );

        assert_eq!(map.ext_of(HASH), "bin");
        assert_eq!(map.ext_of(OTHER_HASH), "bin");
    }

    /// The blobs come in hash order, each with its files lowest `tsid` first and under the
    /// namer's extension, whatever order the rows came in; a file that names no blob is in
    /// no group.
    #[test]
    fn blobs_group_the_files_by_hash_lowest_tsid_first() {
        let map = FileMap::from_files(
            &owner(),
            [
                row("0032SSN7Q4EVJ", &blob(OWNER, HASH), "application/pdf"),
                row("0032SSN7Q4EVK", &blob(OWNER, OTHER_HASH), "text/plain"),
                row("0032SSN7Q4EVG", &blob(OWNER, HASH), "image/png"),
                row("0032SSN7Q4EVM", "https://example.com/a.png", "image/png"),
            ],
        );

        assert_eq!(
            map.blobs(),
            vec![
                Blob {
                    hash: OTHER_HASH.to_string(),
                    ext: "txt".to_string(),
                    tsids: vec!["0032SSN7Q4EVK".to_string()],
                },
                Blob {
                    hash: HASH.to_string(),
                    ext: "png".to_string(),
                    tsids: vec!["0032SSN7Q4EVG".to_string(), "0032SSN7Q4EVJ".to_string()],
                },
            ]
        );
        assert_eq!(map.blobs()[1].file_id(), format!("{HASH}.png"));
        assert!(FileMap::default().blobs().is_empty());
    }

    #[test]
    fn a_src_that_is_not_the_owners_blob_is_left_out() {
        let map = FileMap::from_files(
            &owner(),
            [
                // Another user's blob
                row("0032SSN7Q4EVG", &blob(OTHER, OTHER_HASH), "image/png"),
                // The owner's, but not a blob
                row(
                    "0032SSN7Q4EVH",
                    &format!("pubky://{OWNER}/pub/pubky.app/files/0032SSN7Q4EVG"),
                    "image/png",
                ),
                // Not a pubky address
                row("0032SSN7Q4EVJ", "https://example.com/a.png", "image/png"),
                // Nothing the v0 parser reads
                row("0032SSN7Q4EVK", "", "image/png"),
            ],
        );

        for tsid in [
            "0032SSN7Q4EVG",
            "0032SSN7Q4EVH",
            "0032SSN7Q4EVJ",
            "0032SSN7Q4EVK",
        ] {
            assert_eq!(map.hash_of(tsid), None);
        }
        assert_eq!(map.ext_of(OTHER_HASH), "bin");
    }
}
