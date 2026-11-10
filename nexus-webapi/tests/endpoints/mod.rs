use crate::utils::{host_url, server::TestServiceServer};

use anyhow::Result;
use axum::http::Method;

mod middleware;
mod openapi;
mod versions;

/// The three documents the Swagger UI offers, each at the address it is served from.
const OPENAPI_DOCUMENTS: [&str; 3] = [
    "/api-docs/v0/openapi.json",
    "/api-docs/v1/openapi.json",
    "/api-docs/static/openapi.json",
];

#[tokio_shared_rt::test(shared)]
async fn test_swagger_ui() -> Result<()> {
    let client = httpc_test::new_client(host_url().await)?;

    let res = client.do_get("/swagger-ui").await?;
    assert_eq!(res.status(), 200);
    let body = res.text_body()?;
    assert!(body.contains("<html"));

    // The UI's own configuration lists every document, so each version can be browsed.
    let res = client.do_get("/swagger-ui/swagger-initializer.js").await?;
    assert_eq!(res.status(), 200);
    let config = res.text_body()?;
    for document in OPENAPI_DOCUMENTS {
        assert!(
            config.contains(document),
            "{document} missing from {config}"
        );
    }

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_openapi_schema() -> Result<()> {
    let client = httpc_test::new_client(host_url().await)?;

    for document in OPENAPI_DOCUMENTS {
        let res = client.do_get(document).await?;
        assert_eq!(res.status(), 200, "{document}");

        let body = res.json_body()?;
        assert!(body["openapi"].is_string(), "{document}");
        assert!(body["info"]["title"].is_string(), "{document}");
        assert_eq!(body["info"]["version"], env!("CARGO_PKG_VERSION"));
        assert!(body["paths"].is_object(), "{document}");
    }

    // A version's document describes that version's mount and nothing else.
    for (document, prefix) in [
        ("/api-docs/v0/openapi.json", "/v0/"),
        ("/api-docs/v1/openapi.json", "/v1/"),
    ] {
        let body = client.do_get(document).await?.json_body()?;
        let paths: Vec<_> = body["paths"].as_object().expect("paths").keys().collect();
        assert!(!paths.is_empty(), "{document} lists no path");
        assert!(
            paths.iter().all(|path| path.starts_with(prefix)),
            "{document}: {paths:?}"
        );
    }

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_info_endpoint() -> Result<()> {
    let client = httpc_test::new_client(host_url().await)?;

    let res = client.do_get("/v0/info").await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    println!("body: {body:?}");
    assert_eq!(body["name"], env!("CARGO_PKG_NAME"));
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_pkarr_endpoint() -> Result<()> {
    let test_server = TestServiceServer::get_test_server_with_key_republisher().await;
    let pubky_tls_dns_url = test_server.nexus_api.pubky_tls_dns_url();

    let sdk = test_server.testnet.sdk()?;
    let response = sdk
        .client()
        .request(Method::GET, &format!("{pubky_tls_dns_url}/v0/info"))
        .send()
        .await?;

    assert_eq!(response.status(), 200);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_events_endpoint() -> Result<()> {
    let test_server = TestServiceServer::get_test_server_with_key_republisher().await;
    let pubky_tls_dns_url = test_server.nexus_api.pubky_tls_dns_url();

    let client = &test_server.testnet.client_builder().build()?;

    let response = client
        .request(
            Method::GET,
            &format!("{pubky_tls_dns_url}/v0/events?limit=1000"),
        )
        .send()
        .await?;

    assert_eq!(response.status(), 200);

    let whole_body = response.text().await?;
    let whole_body_lines: Vec<&str> = whole_body.lines().collect();
    let mut whole_line_index = 0usize;
    assert!(whole_body.lines().count() - 1 <= 1000);

    let limit = 10;
    let mut cursor = String::from("");
    let mut counter = 1;

    while limit * counter <= 1000 {
        let response = client
            .request(
                Method::GET,
                &format!("{pubky_tls_dns_url}/v0/events?{cursor}limit={limit}"),
            )
            .send()
            .await?;
        assert_eq!(response.status(), 200);

        let part_of_body = response.text().await?;
        assert!(part_of_body.lines().count() - 1 <= limit);

        cursor = part_of_body
            .lines()
            .last()
            .and_then(|line| line.strip_prefix("cursor: "))
            .map(|s| format!("cursor={}&", s))
            .unwrap_or_default();

        let lines: Vec<&str> = part_of_body.lines().collect();
        let count = lines.len();
        for &line in lines.iter().take(count - 1) {
            assert!(
                whole_body_lines[whole_line_index..]
                    .iter()
                    .position(|candidate| candidate == &line)
                    .map(|pos| {
                        whole_line_index += pos + 1;
                        true
                    })
                    .unwrap_or(false),
                "Line `{line}` missing or out of order in complete response"
            );
        }
        counter += 1;
    }

    Ok(())
}
