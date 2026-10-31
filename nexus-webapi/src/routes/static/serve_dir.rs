use std::path::PathBuf;
use std::sync::OnceLock;

use crate::media::{MediaFile, VariantController};
use crate::Result;
use axum::{
    body::Body,
    http::{header, HeaderValue, Request, Uri},
    response::Response,
};
use nexus_common::media::FileVariant;
use pubky_social_specs::essence;
use tower_http::services::{fs::ServeFileSystemResponseBody, ServeDir};
use tracing::error;

static SERVE_DIR_INSTANCE: OnceLock<ServeDir> = OnceLock::new();

const CACHE_CONTROL: &str = "public, max-age=3600";

/// The content types a browser may render in place. Everything served from the Nexus host runs
/// with its origin, so a file is only shown inline when its type cannot carry code: no HTML, no
/// SVG (it embeds scripts), no script or stylesheet types, no XML. Essences, as [`essence`]
/// returns them: lowercase, no parameters.
const INLINE_CONTENT_TYPES: &[&str] = &[
    "image/jpeg",
    "image/png",
    "image/gif",
    "image/webp",
    "video/mp4",
    "video/mpeg",
    "audio/mpeg",
    "audio/wav",
    "application/pdf",
    "text/plain",
];

/// Whether a file served under `content_type` may display in the browser, or must be offered as
/// a download. The type is compared on its essence, so `Text/HTML; charset=x` is HTML and
/// downloads, and a malformed type downloads too: a type that cannot be read is not one of the
/// safe ones. The caller passes the type of what is served, not what was uploaded: a derived
/// variant of an SVG is WebP, and that one displays.
fn is_served_inline(content_type: &str) -> bool {
    essence(content_type).is_some_and(|essence| INLINE_CONTENT_TYPES.contains(&essence.as_str()))
}

pub struct PubkyServeDir;

/// Wrapper around ServeDir to serve files from the configured directory
impl PubkyServeDir {
    fn get_serve_dir(file_path: PathBuf) -> ServeDir {
        SERVE_DIR_INSTANCE
            .get_or_init(|| ServeDir::new(file_path))
            .to_owned()
    }

    pub async fn try_call(
        mut request: Request<Body>,
        path: String,
        content_type: String,
        file_path: PathBuf,
    ) -> Result<Response<ServeFileSystemResponseBody>> {
        *request.uri_mut() = path.as_str().parse::<Uri>()?;

        let response_result = Self::get_serve_dir(file_path).try_call(request).await;

        let mut response = match response_result {
            // In case of success, extract the response for content-type header injection
            // is_success() checks for all success codes: 200 OK, but also 206 Partial Content
            // 206 is used for video streaming
            Ok(response) if response.status().is_success() => response,

            // In all other cases, return right away and skip content-type header injection
            Ok(response) => return Ok(response),
            Err(err) => return Err(err.into()),
        };

        // set the content type header
        let content_type_header = content_type
            .parse()
            .inspect_err(|_| error!("Invalid content type header: {}", content_type))?;

        response
            .headers_mut()
            .insert("content-type", content_type_header);
        Ok(response)
    }
}

/// Ensures a file variant exists, serves it, and applies cache headers.
///
/// The one place a file's bytes leave the host, so the rule that keeps user content from running
/// here is applied to every file route: every response carries `X-Content-Type-Options: nosniff`,
/// and a variant whose served type is not one of [`INLINE_CONTENT_TYPES`] is offered as a bare
/// `Content-Disposition: attachment`, with no filename, since a non-ASCII name would not fit the
/// header. If `download` is true (`?dl=`), the response is an attachment under the file's download
/// name when it has one, whatever its type.
pub async fn serve_file_variant(
    request: Request<Body>,
    file: &MediaFile,
    variant: &FileVariant,
    files_path: PathBuf,
    download: bool,
    controller: &VariantController,
) -> Result<Response<ServeFileSystemResponseBody>> {
    let content_type = controller
        .ensure_variant(file, variant, &files_path)
        .await?;
    let inline = is_served_inline(&content_type);

    // `ServeDir` is rooted at `files_path`, so the request path is the file's folder under it.
    let disk_path = format!("/{}/{variant}", file.folder().display());

    let mut response =
        PubkyServeDir::try_call(request, disk_path, content_type, files_path).await?;

    let headers = response.headers_mut();
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(CACHE_CONTROL),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );

    if download || !inline {
        let content_disposition = match &file.download_name {
            Some(filename) if download => {
                let named = format!("attachment; filename=\"{filename}\"");
                named
                    .parse()
                    .inspect_err(|_| error!("Invalid content disposition header: {named}"))?
            }
            _ => HeaderValue::from_static("attachment"),
        };
        headers.insert(header::CONTENT_DISPOSITION, content_disposition);
    }

    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Every type a browser may render in place displays, however the declared type is cased or
    // parameterised: the rule reads the essence.
    #[test]
    fn test_inline_types_display() {
        for content_type in INLINE_CONTENT_TYPES {
            assert!(is_served_inline(content_type), "{content_type}");
        }
        for content_type in ["Image/PNG", "image/png; charset=x", "VIDEO/MP4;codecs=avc1"] {
            assert!(is_served_inline(content_type), "{content_type}");
        }
    }

    // Anything that can carry code, anything not on the list, and anything that cannot be read
    // as a type at all is offered as a download.
    #[test]
    fn test_active_unknown_and_malformed_types_download() {
        for content_type in [
            "text/html",
            "Text/HTML; charset=x",
            "image/svg+xml",
            "application/javascript",
            "text/css",
            "application/xml",
            "text/xml",
            "application/json",
            "application/octet-stream",
            "image/heic",
            "",
            "garbage",
            " image/png",
            "image/png/x",
        ] {
            assert!(!is_served_inline(content_type), "{content_type:?}");
        }
    }
}
