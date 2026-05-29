use crate::stream::post::utils::verify_post_list;
use crate::stream::post::BOGOTA;
use crate::stream::post::ROOT_PATH;
use crate::utils::get_request;
use anyhow::Result;

const POST_A1: &str = "SIJW1TGL5BKG6";
const POST_A2: &str = "GJMW1TGL5BKG6";
const POST_A3: &str = "MLOW1TGL5BKH6";
const POST_A4: &str = "SIJW1TGL5BKG4";
const POST_A5: &str = "GJMW1TGL5BKG4";
const POST_A6: &str = "MLOW1TGL5BKH4";
const POST_A7: &str = "SIJW1TGL5BKG1";
const POST_A8: &str = "GJMW1TGL5BKG1";
const POST_A9: &str = "MLOW1TGL5BKH1";
const COL_BOGOTA_2: &str = "COLW1TGL5BKG2";
const COL_BOGOTA_1: &str = "COLW1TGL5BKG1";
const COL_BOGOTA_MALF: &str = "MALF1TGL5BKG7";
const COL_BOGOTA_NEST: &str = "NEST1TGL5BKG8";
const POST_A10: &str = "5YCW1TGL5BKG6";

const POST_W_PUBKY_TAG_1: &str = "5YCW1TGL5BKG1";
const POST_W_PUBKY_TAG_2: &str = "4ZCW1TGL5BKG1";

pub const START_TIMELINE: &str = "1980477299341";
pub const END_TIMELINE: &str = "1980477299303";

pub const PUBKY_TAG: &str = "pubky";
pub const FK_TAG: &str = "4k";

#[tokio_shared_rt::test(shared)]
async fn test_stream_post_kind() -> Result<()> {
    // limit=14 to bound the assertion to a deterministic prefix — Bogota
    // authored 22 root posts (incl. 4 Collections); the list below is the
    // top of that by indexed_at DESC.
    let path = format!("{ROOT_PATH}?author_id={BOGOTA}&source=author&limit=14");

    let body = get_request(&path).await?;
    let post_list = vec![
        COL_BOGOTA_NEST,
        COL_BOGOTA_MALF,
        POST_A1,
        POST_A2,
        POST_A3,
        POST_A4,
        POST_A5,
        POST_A6,
        POST_A7,
        POST_A8,
        POST_A9,
        COL_BOGOTA_2,
        COL_BOGOTA_1,
        POST_A10,
    ];
    verify_post_list(post_list, body);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_post_kind_with_start_and_end() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?author_id={BOGOTA}&source=author&start={START_TIMELINE}&end={END_TIMELINE}"
    );

    let body = get_request(&path).await?;
    let post_list = vec![POST_A3, POST_A4, POST_A5, POST_A6, POST_A7];
    verify_post_list(post_list, body);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_post_kind_with_tag() -> Result<()> {
    let path = format!("{ROOT_PATH}?author_id={BOGOTA}&source=author&tags={PUBKY_TAG}");

    let body = get_request(&path).await?;
    let post_list = vec![POST_A4, POST_A9, POST_W_PUBKY_TAG_1, POST_W_PUBKY_TAG_2];
    verify_post_list(post_list, body);

    Ok(())
}
