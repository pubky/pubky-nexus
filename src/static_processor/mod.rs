use axum::Error;

use crate::models::file::{details::FileVersions, FileDetails};

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
