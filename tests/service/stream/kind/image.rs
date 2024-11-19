use crate::service::stream::utils::verify_post_list;
use crate::service::stream::ROOT_PATH;
use crate::service::utils::make_request;
use anyhow::Result;

const BOGOTA: &str = "ep441mndnsjeesenwz78r9paepm6e4kqm4ggiyy9uzpoe43eu9ny";

const POST_I1: &str = "5YCW1TGL5BKG8";
const POST_I2: &str = "5YCW1TGL5BKG7";
const POST_I3: &str = "5YCW1TGL5BKG6";
const POST_I4: &str = "5YCW1TGL5BKG5";
const POST_I5: &str = "5YCW1TGL5BKG4";
const POST_I6: &str = "5YCW1TGL5BKG3";
const POST_I7: &str = "5YCW1TGL5BKG2";
const POST_I8: &str = "5YCW1TGL5BKG1";

pub const START_TIMELINE: &str = "1820477299345";
pub const END_TIMELINE: &str = "1820477299308";

#[tokio::test]
async fn test_stream_image_post_kind() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind=image");

    let body = make_request(&path).await?;
    let post_list = vec![
        POST_I1, POST_I2, POST_I3, POST_I4, POST_I5, POST_I6, POST_I7, POST_I8,
    ];
    verify_post_list(post_list, body);

    Ok(())
}

#[tokio::test]
async fn test_stream_image_post_kind_with_start() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind=image&start={START_TIMELINE}");

    let body = make_request(&path).await?;
    let post_list = vec![POST_I3, POST_I4, POST_I5, POST_I6, POST_I7, POST_I8];
    verify_post_list(post_list, body);

    Ok(())
}

#[tokio::test]
async fn test_stream_image_post_kind_with_end() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind=image&end={START_TIMELINE}");

    let body = make_request(&path).await?;
    let post_list = vec![POST_I1, POST_I2];
    verify_post_list(post_list, body);

    Ok(())
}

#[tokio::test]
async fn test_stream_image_post_kind_with_start_and_end() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind=image&start={START_TIMELINE}&end={END_TIMELINE}");

    let body = make_request(&path).await?;
    let post_list = vec![POST_I3, POST_I4, POST_I5, POST_I6];
    verify_post_list(post_list, body);

    Ok(())
}

#[tokio::test]
async fn test_stream_image_post_kind_with_author() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind=image&author_id={BOGOTA}&source=author");

    let body = make_request(&path).await?;
    let post_list = vec![POST_I3, POST_I5, POST_I8];
    verify_post_list(post_list, body);

    Ok(())
}

#[tokio::test]
async fn test_stream_image_post_kind_with_author_skip_and_limit() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind=image&author_id={BOGOTA}&source=author&skip=1&limit=1");

    let body = make_request(&path).await?;
    let post_list = vec![POST_I5];
    verify_post_list(post_list, body);

    Ok(())
}
