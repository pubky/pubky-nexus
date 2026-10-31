//! Shared media vocabulary: the variant names, the table of which variants a content type has,
//! and the folder a file lives in on disk.
//!
//! Deriving a variant lives in the API (`nexus-webapi`), the only service that does it, and so
//! does the label a derived variant is served under -- that one belongs beside the processor
//! that produces the bytes. What stays here is what both services must agree on: a file's variant
//! URLs are built from the same table the API validates requests against, and the watcher writes
//! a file into the same folder the API serves it from.

use crate::types::DynError;
use serde::{Deserialize, Serialize};
use std::{
    fmt::Display,
    path::{Path, PathBuf},
    str::FromStr,
};
use utoipa::ToSchema;

/// The folder under `files_path` that holds every `social/v1` file, one owner folder inside it.
/// It cannot clash with an owner folder: an owner id is 52 characters.
pub const V1_FILES_DIR: &str = "v1";

/// The folder a file's variants live in, relative to `files_path`. The one place the layout is
/// decided: the watcher writes a file through it and the API reads through it, and no test
/// crosses the two, so this is what keeps them in agreement.
///
/// The id form picks the folder. A `{tsid}`, with no dot, is a v0 file at `<owner>/<tsid>/`. A
/// `{hash}.{ext}`, with a dot, is a `social/v1` file at `v1/<owner>/{hash}.{ext}/`. The extension
/// stays in the folder name because `H.png` and `H.bin` are two files on the homeserver: a DEL
/// of one must leave the other, and a folder exists only for an address that was uploaded.
pub fn file_folder(owner_id: &str, file_id: &str) -> PathBuf {
    let owner_folder = if is_v1_file_id(file_id) {
        Path::new(V1_FILES_DIR).join(owner_id)
    } else {
        PathBuf::from(owner_id)
    };
    owner_folder.join(file_id)
}

/// Whether `file_id` names a `social/v1` file: a `{hash}.{ext}`, with a dot, against a v0
/// `{tsid}` without one. The one rule [`file_folder`] picks the folder by, shared so a caller
/// that must know which kind of file an id names (a v1 file has no `File` node to read) cannot
/// disagree with the folder it lives in.
pub fn is_v1_file_id(file_id: &str) -> bool {
    file_id.contains('.')
}

#[derive(Debug, PartialEq, Serialize, Deserialize, ToSchema, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum FileVariant {
    Main,
    Large,
    Feed,
    Small,
}

impl FromStr for FileVariant {
    type Err = DynError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "main" => Ok(FileVariant::Main),
            "large" => Ok(FileVariant::Large),
            "feed" => Ok(FileVariant::Feed),
            "small" => Ok(FileVariant::Small),
            _ => Err("Invalid file version".into()),
        }
    }
}

impl Display for FileVariant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let version_string = match self {
            FileVariant::Main => "main",
            FileVariant::Large => "large",
            FileVariant::Feed => "feed",
            FileVariant::Small => "small",
        };
        write!(f, "{version_string}")
    }
}

/// The family of media a content type belongs to: the one thing every table keyed on a content
/// type -- which variants it has, what a derived variant is served as, which processor derives
/// it -- agrees on. Answered here once so the tables cannot drift apart on what counts as an
/// image. Matching is on the top-level type only (`image/…`, `video/…`) and ASCII
/// case-insensitive, as RFC 2045 requires, so `imagefoo` is no kind and `Image/png` is an image.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum MediaKind {
    Image,
    Video,
}

impl MediaKind {
    /// The kind a content type names, by its top-level type (`image/…`, `video/…`). `None` for
    /// anything else: nothing derives from it.
    ///
    /// Content types are compared case-insensitively, as RFC 2045 requires, so `Image/png` is an
    /// image.
    pub fn from_content_type(content_type: &str) -> Option<Self> {
        let (top_level, _subtype) = content_type.split_once('/')?;
        if top_level.eq_ignore_ascii_case("image") {
            Some(Self::Image)
        } else if top_level.eq_ignore_ascii_case("video") {
            Some(Self::Video)
        } else {
            None
        }
    }
}

/// Variants a content type can be served as, `Main` included. Empty for a content type with no
/// variants at all, which is also how an unsupported one answers.
pub fn get_valid_variants_for_content_type(content_type: &str) -> Vec<FileVariant> {
    match MediaKind::from_content_type(content_type) {
        // Largest to smallest.
        Some(MediaKind::Image) => vec![
            FileVariant::Main,
            FileVariant::Large,
            FileVariant::Feed,
            FileVariant::Small,
        ],
        Some(MediaKind::Video) => vec![FileVariant::Main],
        None => vec![],
    }
}

/// Whether this variant is one the content type has. `Main` always is: it is the upload itself.
pub fn validate_variant_for_content_type(content_type: &str, variant: &FileVariant) -> bool {
    if variant == &FileVariant::Main {
        return true;
    }
    get_valid_variants_for_content_type(content_type).contains(variant)
}

#[cfg(test)]
mod tests {
    use super::*;

    // The id form alone decides where a file lives: a `{tsid}` under the owner, a `{hash}.{ext}`
    // under `v1/<owner>`, with the extension kept so two addresses never share a folder.
    #[test]
    fn test_file_folder_is_picked_by_the_id_form() {
        assert_eq!(
            file_folder("owner", "0032SSN7Q4EVG"),
            Path::new("owner/0032SSN7Q4EVG")
        );
        assert_eq!(
            file_folder("owner", "0032SSN7Q4EVG.png"),
            Path::new("v1/owner/0032SSN7Q4EVG.png")
        );
        assert_eq!(
            file_folder("owner", "0032SSN7Q4EVG.bin"),
            Path::new("v1/owner/0032SSN7Q4EVG.bin")
        );
        assert!(!is_v1_file_id("0032SSN7Q4EVG"));
        assert!(is_v1_file_id("0032SSN7Q4EVG.png"));
    }

    // The kind is read off the top-level type, so a content type that only starts with the word
    // is not an image: it must get no derived variants, or the API would be asked for one it
    // has no processor for. The type is compared ignoring case (RFC 2045), so `Image/png` is
    // still an image.
    #[test]
    fn test_media_kind_matches_the_top_level_type_only_ignoring_case() {
        for content_type in ["image/png", "Image/png", "IMAGE/PNG"] {
            assert_eq!(
                MediaKind::from_content_type(content_type),
                Some(MediaKind::Image),
                "{content_type}"
            );
            assert!(validate_variant_for_content_type(
                content_type,
                &FileVariant::Small
            ));
        }
        for content_type in ["video/mp4", "Video/mp4", "VIDEO/MP4"] {
            assert_eq!(
                MediaKind::from_content_type(content_type),
                Some(MediaKind::Video),
                "{content_type}"
            );
            assert_eq!(
                get_valid_variants_for_content_type(content_type),
                vec![FileVariant::Main]
            );
        }
        for content_type in ["imagefoo", "videofoo", "image", "application/pdf", ""] {
            assert_eq!(MediaKind::from_content_type(content_type), None);
            assert!(get_valid_variants_for_content_type(content_type).is_empty());
            assert!(!validate_variant_for_content_type(
                content_type,
                &FileVariant::Small
            ));
        }
    }

    #[test]
    fn test_unsupported_content_type_has_no_variants() {
        assert!(get_valid_variants_for_content_type("application/pdf").is_empty());
        assert!(!validate_variant_for_content_type(
            "application/pdf",
            &FileVariant::Small
        ));
        // `main` is the upload itself, so it is valid even with nothing to derive from it.
        assert!(validate_variant_for_content_type(
            "application/pdf",
            &FileVariant::Main
        ));
    }

    #[test]
    fn test_image_has_derived_variants_and_video_does_not() {
        assert_eq!(
            get_valid_variants_for_content_type("image/jpeg"),
            vec![
                FileVariant::Main,
                FileVariant::Large,
                FileVariant::Feed,
                FileVariant::Small,
            ]
        );
        assert_eq!(
            get_valid_variants_for_content_type("video/mp4"),
            vec![FileVariant::Main]
        );
        assert!(!validate_variant_for_content_type(
            "video/mp4",
            &FileVariant::Small
        ));
    }
}
