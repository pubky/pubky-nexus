use crate::models::{FileId, PubkyId};
use crate::{Error, Result};
use nexus_common::media::is_v1_file_id;
use pubky_social_specs::{file_uri_builder, ParsedUri, Resource};

/// A file id as `/v1/static/files` takes it, in either form, so a client has one base path for
/// all file bytes and never picks a route from an address.
///
/// The form is decided by [`is_v1_file_id`], the rule the file's folder is picked by, and each
/// form is checked by its own parser. A `{tsid}` is a v0 file with a `File` node, validated as
/// `/static/files` validates it. A `{hash}.{ext}` is a `social/v1` file with no node, validated
/// by the spec crate's reader of a file address, the one that decides what a client could have
/// uploaded: `H.JPG`, `H.tar.gz` and a bare `H` are not file addresses there, so they are not
/// file ids here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileRef {
    /// A v0 file, by its 13-character id.
    Legacy(FileId),
    /// A `social/v1` file, by its full `{hash}.{ext}` and the extension alone, which is all its
    /// address says about the type of its bytes.
    V1 { filename: String, ext: String },
}

impl FileRef {
    /// Reads `file_id` as the id of a file of `owner_id`. The owner is needed to put a v1 id
    /// through the crate's reader, which reads whole addresses; the id it hands back must be
    /// the raw one, so nothing the reader normalises can slip past the check.
    pub fn parse(owner_id: &PubkyId, file_id: String) -> Result<Self> {
        if !is_v1_file_id(&file_id) {
            return FileId::try_from(file_id).map(Self::Legacy);
        }

        let uri = file_uri_builder(owner_id.to_string(), file_id.clone());
        match ParsedUri::try_from(uri.as_str()).map(|parsed| parsed.resource) {
            Ok(Resource::File(filename)) if filename == file_id => {
                // The dot is there by the check above, and the reader accepted exactly one
                // extension after the last one.
                let ext = filename.rsplit_once('.').map_or("", |(_, ext)| ext).to_string();
                Ok(Self::V1 { filename, ext })
            }
            _ => Err(Error::invalid_input(format!(
                "Invalid file id: {file_id} is neither a 13-character id nor a {{hash}}.{{ext}} file name"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWNER: &str = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";
    /// A canonical hash id: blake3 over `[1, 2]`, first 16 bytes, Crockford.
    const HASH: &str = "PZBQ010FF079VVZPQG1RNFN6DR";

    fn owner() -> PubkyId {
        PubkyId::try_from(OWNER).expect("owner id")
    }

    fn parse(file_id: &str) -> Result<FileRef> {
        FileRef::parse(&owner(), file_id.to_string())
    }

    // Each form goes to its own parser: a dotless id is a 13-character v0 id, a dotted one is
    // a v1 file name whose extension is read off for the content type.
    #[test]
    fn test_both_id_forms_are_read() {
        assert!(
            matches!(parse("2ZK2H8P2T5NG0"), Ok(FileRef::Legacy(id)) if &*id == "2ZK2H8P2T5NG0")
        );
        for ext in ["png", "bin", "json", "xml"] {
            let filename = format!("{HASH}.{ext}");
            assert_eq!(
                parse(&filename).expect(&filename),
                FileRef::V1 {
                    filename: filename.clone(),
                    ext: ext.to_string()
                }
            );
        }
    }

    // What the crate's reader does not accept as a file address is not a file id: a cased or
    // unknown extension, two extensions, no extension on a hash, a hash of the wrong shape, and
    // a path that is not one segment. A v0 id is held to the v0 rule.
    #[test]
    fn test_ids_no_client_could_upload_are_invalid_input() {
        let rejected = [
            format!("{HASH}.JPG"),
            format!("{HASH}.tar.gz"),
            HASH.to_string(),
            format!("{HASH}."),
            ".png".to_string(),
            format!("{}.png", &HASH[..25]),
            "2ZK2H8P2T5NG0.png".to_string(),
            format!("{HASH}.png/main"),
            format!("../{HASH}.png"),
            "ABCDEFG".to_string(),
            String::new(),
        ];
        for file_id in rejected {
            assert!(
                matches!(parse(&file_id), Err(Error::InvalidInput { .. })),
                "{file_id:?}"
            );
        }
    }
}
