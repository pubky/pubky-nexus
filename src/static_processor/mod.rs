use axum::Error;

use crate::models::file::{
    details::{FileUrls, FileVersions},
    FileDetails,
};

pub mod image;
pub mod store;
pub mod video;

pub async fn create_file_version(file: &FileDetails, version: FileVersions) -> Result<(), Error> {
    match &file.content_type {
        content_type if content_type.starts_with("image/") => {
            image::create_image_version(file, version).await
        }
        content_type if content_type.starts_with("video/") => {
            video::create_video_version(file, version).await
        }
        _ => Ok(()),
    }
}

pub fn get_file_urls_by_content_type(content_type: &str, path: &str) -> FileUrls {
    let versions = match content_type {
        value if value.starts_with("image") => image::get_image_versions(content_type),
        value if value.starts_with("video") => video::get_video_versions(),
        _ => vec![],
    };

    FileUrls {
        main: format!("{}/main", path),
        feed: versions
            .contains(&FileVersions::FEED)
            .then_some(format!("{}/feed", path)),
        small: versions
            .contains(&FileVersions::SMALL)
            .then_some(format!("{}/small", path)),
    }
}
