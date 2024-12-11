use std::io::{Error as IoError, ErrorKind};
use tokio::process::Command;

use crate::{
    models::file::{details::FileVersions, FileDetails},
    types::DynError,
};

use super::store::get_storage_path;

struct VideoOptions {
    width: String,
    format: String,
    content_type: String,
}

fn get_video_option_for_version(_version: &FileVersions) -> Option<VideoOptions> {
    return None;
    // let width = match version {
    //     FileVersions::FEED => String::from("720"),
    //     _ => return None,
    // };
    // Some(VideoOptions {
    //     format: "mp4".to_string(),
    //     width,
    // })
}

pub async fn create_video_version(
    file: &FileDetails,
    version: FileVersions,
) -> Result<String, DynError> {
    let video_options = get_video_option_for_version(&version);

    if video_options.is_none() {
        return Err(format!("bad video version: {:?}", version).into());
    }

    let input_path = format!(
        "{}/{}/{}/{}",
        get_storage_path(),
        file.owner_id,
        file.id,
        "main"
    );

    let options = video_options.unwrap();

    match process_video(
        &input_path,
        version,
        options.width.as_str(),
        Some(options.format.as_str()),
    )
    .await
    {
        Ok(_) => Ok(options.content_type),
        Err(err) => Err(format!("Failed to process video: {}", err).into()),
    }
}

pub async fn process_video(
    input: &str,
    version: FileVersions,
    width: &str,
    output_format: Option<&str>,
) -> Result<String, IoError> {
    let output_path = format!("{}/{}", input, version);
    let output = match output_format {
        None => output_path,
        Some(format) => format!("{}.{}", output_path, format),
    };
    let child_output = Command::new("ffmpeg")
        .arg("-i")
        .arg(input)
        .arg("-vf")
        .arg(&format!("scale={}:-1", width))
        .arg("-c:a")
        .arg("copy")
        .arg(output)
        .output() // Automatically pipes stdout and stderr
        .await?;

    if child_output.status.success() {
        Ok(String::from_utf8_lossy(&child_output.stdout).to_string())
    } else {
        Err(IoError::new(
            ErrorKind::Other,
            format!(
                "FFmpeg command failed: {}",
                String::from_utf8_lossy(&child_output.stderr)
            ),
        ))
    }
}

// pub async fn get_video_format(input: &str) -> Result<String, IoError> {
//     let child_output = Command::new("ffmpeg")
//         .arg("-i")
//         .arg(input)
//         .arg("-f")
//         .arg("null")
//         .output() // Automatically pipes stdout and stderr
//         .await?;

//     if child_output.status.success() {
//         Ok(String::from_utf8_lossy(&child_output.stderr).to_string()) // Metadata is usually on stderr
//     } else {
//         Err(IoError::new(
//             ErrorKind::Other,
//             format!(
//                 "FFmpeg metadata extraction failed: {}",
//                 String::from_utf8_lossy(&child_output.stderr)
//             ),
//         ))
//     }
// }

pub fn get_video_versions() -> Vec<FileVersions> {
    vec![]
}
