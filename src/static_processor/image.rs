use axum::Error;
use std::io::{Error as IoError, ErrorKind};
use tokio::process::Command;

use crate::models::file::{details::FileVersions, FileDetails};

use super::store::get_storage_path;

struct ImageOptions {
    width: String,
    format: String,
}

fn get_image_option_for_version(
    file: &FileDetails,
    version: &FileVersions,
) -> Option<ImageOptions> {
    let format = match file.content_type.as_str() {
        "image/gif" => String::from("gif"),
        _ => String::from("jpeg"),
    };
    let width = match version {
        FileVersions::SMALL => String::from("320"),
        FileVersions::FEED => String::from("720"),
        _ => return None,
    };
    Some(ImageOptions { format, width })
}

pub async fn create_image_version(file: &FileDetails, version: FileVersions) -> Result<(), Error> {
    let image_options = get_image_option_for_version(&file, &version);

    if image_options.is_none() {
        return Ok(());
    }

    let input_path = format!(
        "{}/{}/{}/{}",
        get_storage_path(),
        file.owner_id,
        file.id,
        FileVersions::MAIN
    );

    let options = image_options.unwrap();

    let image_ext = get_image_format(&input_path).await.unwrap();

    let output_format = match image_ext.as_str() {
        "JPEG" => None,
        _ => Some(options.format.as_str()),
    };

    match process_image(&input_path, version, options.width.as_str(), output_format).await {
        Ok(_) => Ok(()),
        Err(err) => Err(Error::new(err)),
    }
}

// Async function to run ImageMagick command to resize and format conversion
pub async fn process_image(
    input: &str,
    version: FileVersions,
    width: &str,
    output_format: Option<&str>,
) -> Result<String, IoError> {
    let output_path = format!("{}/{}", input, version);
    let output = match output_format {
        None => output_path,
        Some(format) => format!("{}:{}", format, output_path),
    };
    let child_output = Command::new("convert")
        .arg(input)
        .arg("-background")
        .arg("white")
        .arg("-alpha")
        .arg("remove")
        .arg("-resize")
        .arg(&format!("{}x", width))
        .arg(output)
        .output() // Automatically pipes stdout and stderr
        .await?;

    if child_output.status.success() {
        Ok(String::from_utf8_lossy(&child_output.stdout).to_string())
    } else {
        Err(IoError::new(
            ErrorKind::Other,
            format!(
                "ImageMagick command failed: {}",
                String::from_utf8_lossy(&child_output.stderr)
            ),
        ))
    }
}

// Async function to get image format
pub async fn get_image_format(input: &str) -> Result<String, IoError> {
    let child_output = Command::new("identify")
        .arg("-format")
        .arg("%m")
        .arg(input)
        .output() // Automatically pipes stdout and stderr
        .await?;

    if child_output.status.success() {
        Ok(String::from_utf8_lossy(&child_output.stdout).to_string())
    } else {
        Err(IoError::new(
            ErrorKind::Other,
            format!(
                "ImageMagick format extraction failed: {}",
                String::from_utf8_lossy(&child_output.stderr)
            ),
        ))
    }
}
