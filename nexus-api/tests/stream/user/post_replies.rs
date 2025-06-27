use anyhow::Result;

use axum::http::StatusCode;

use crate::utils::{get_request, invalid_get_request};

#[tokio_shared_rt::test(shared)]
async fn test_stream_users_for_post_replies() -> Result<()> {
    // List of unique replier IDs
    let replier_ids = [
        "f5tcy5gtgzshipr6pag6cn9uski3s8tjare7wd3n7enmyokgjk1o",
        "8attbeo9ftu5nztqkcfw3gydksehr7jbspgfi64u4h8eo5e7dbiy",
        "7w4hmktqa7gia5thmk7zki8px7ttwpwjtgaaaou4tbqx64re8d1o",
        "ep441mndnsjeesenwz78r9paepm6e4kqm4ggiyy9uzpoe43eu9ny",
    ];

    let author_id = "emq37ky6fbnaun7q1ris6rx3mqmw3a33so1txfesg9jj3ak9ryoy";
    let post_id = "1A1P4D8C9K0F";

    let body = get_request(&format!(
        "/v0/stream/users?source=post_replies&post_id={post_id}&author_id={author_id}",
    ))
    .await?;

    let users = body.as_array().expect("User stream should be an array");

    // Check that the correct number of users is returned
    assert_eq!(
        users.len(),
        replier_ids.len(),
        "Expected {} users in the response",
        replier_ids.len()
    );

    // Check that the returned users are the same as the replier IDs
    for user in users {
        let user_id = user["details"]["id"]
            .as_str()
            .expect("User ID should be a string");
        assert!(
            replier_ids.contains(&user_id),
            "User ID {user_id} should be in the list of replier IDs"
        );
    }

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_users_for_post_replies_no_post_id() -> Result<()> {
    let author_id = "emq37ky6fbnaun7q1ris6rx3mqmw3a33so1txfesg9jj3ak9ryoy";

    invalid_get_request(
        &format!("/v0/stream/users?source=post_replies&author_id={author_id}",),
        StatusCode::BAD_REQUEST,
    )
    .await?;

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_users_for_post_replies_no_author_id() -> Result<()> {
    let post_id = "1A1P4D8C9K0F";
    invalid_get_request(
        &format!("/v0/stream/users?source=post_replies&post_id={post_id}",),
        StatusCode::BAD_REQUEST,
    )
    .await?;

    Ok(())
}
