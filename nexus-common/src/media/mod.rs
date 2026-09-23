//! Shared media vocabulary: the variant names, and the table of which variants a content type
//! has.
//!
//! Deriving a variant lives in the API (`nexus-webapi`), the only service that does it, and so
//! does the label a derived variant is served under -- that one belongs beside the processor
//! that produces the bytes. What stays here is what both services must agree on: the watcher
//! publishes a file's variant URLs from the same table the API validates requests against.

use crate::{models::file::FileUrls, types::DynError};
use serde::{Deserialize, Serialize};
use std::{fmt::Display, path::Path, str::FromStr};
use utoipa::ToSchema;

#[derive(Debug, PartialEq, Serialize, Deserialize, ToSchema, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum FileVariant {
    Main,
    Hero,
    Feed,
    Small,
}

impl FromStr for FileVariant {
    type Err = DynError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "main" => Ok(FileVariant::Main),
            "hero" => Ok(FileVariant::Hero),
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
            FileVariant::Hero => "hero",
            FileVariant::Feed => "feed",
            FileVariant::Small => "small",
        };
        write!(f, "{version_string}")
    }
}

/// Variants a content type can be served as, `Main` included. Empty for a content type with no
/// variants at all, which is also how an unsupported one answers.
pub fn get_valid_variants_for_content_type(content_type: &str) -> Vec<FileVariant> {
    match content_type {
        // Largest to smallest.
        value if value.starts_with("image") => {
            vec![
                FileVariant::Main,
                FileVariant::Hero,
                FileVariant::Feed,
                FileVariant::Small,
            ]
        }
        value if value.starts_with("video") => vec![FileVariant::Main],
        _ => vec![],
    }
}

/// The URLs to publish for a file, one per variant its content type has.
pub fn get_file_urls_by_content_type(content_type: &str, path: &Path) -> FileUrls {
    FileUrls::new(path, &get_valid_variants_for_content_type(content_type))
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
                FileVariant::Hero,
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
