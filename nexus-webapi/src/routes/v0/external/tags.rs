use crate::routes::v0::endpoints::{EXTERNAL_TAGGERS_ROUTE, EXTERNAL_TAGS_ROUTE};
use crate::routes::v0::TaggersInfoResponse;
use crate::{Error, Result};
use axum::extract::{Path, Query};
use axum::Json;
use nexus_common::models::link::ExternalLinkDetails;
use nexus_common::models::tag::external::TagExternal;
use nexus_common::models::tag::traits::TagCollection;
use nexus_common::models::tag::TagDetails;
use nexus_common::types::Pagination;
use serde::Deserialize;
use tracing::info;
use utoipa::OpenApi;

#[derive(Deserialize, Debug)]
pub struct ExternalTagsQuery {
    pub url: Option<String>,
    pub id: Option<String>,
    pub limit_tags: Option<usize>,
    pub skip_tags: Option<usize>,
    pub limit_taggers: Option<usize>,
}

#[derive(Deserialize, Debug)]
pub struct ExternalTaggersQuery {
    #[serde(flatten)]
    pub pagination: Pagination,
    pub url: Option<String>,
    pub id: Option<String>,
}

#[utoipa::path(
    get,
    path = EXTERNAL_TAGS_ROUTE,
    description = "External resource tags",
    tag = "External",
    params(
        ("url" = Option<String>, Query, description = "Target URL to inspect"),
        ("id" = Option<String>, Query, description = "Precomputed external link identifier"),
        ("skip_tags" = Option<usize>, Query, description = "Skip N tags. Defaults to 0"),
        ("limit_tags" = Option<usize>, Query, description = "Upper limit on the number of tags. Defaults to 5"),
        ("limit_taggers" = Option<usize>, Query, description = "Upper limit on the number of taggers per tag. Defaults to 5"),
    ),
    responses(
        (status = 200, description = "External tags", body = Vec<TagDetails>),
        (status = 404, description = "Tags not found"),
        (status = 400, description = "Invalid input"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn external_tags_handler(
    Query(query): Query<ExternalTagsQuery>,
) -> Result<Json<Vec<TagDetails>>> {
    let link_id = resolve_external_id(query.url, query.id)?;
    info!(
        "GET {EXTERNAL_TAGS_ROUTE} link_id:{}, skip_tags:{:?}, limit_tags:{:?}, limit_taggers:{:?}",
        link_id, query.skip_tags, query.limit_tags, query.limit_taggers
    );

    match TagExternal::get_by_id(
        &link_id,
        query.skip_tags,
        query.limit_tags,
        query.limit_taggers,
        None,
    )
    .await
    {
        Ok(Some(tags)) => Ok(Json(tags)),
        Ok(None) => Err(Error::TagsNotFound { reach: link_id }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[utoipa::path(
    get,
    path = EXTERNAL_TAGGERS_ROUTE,
    description = "External resource taggers",
    tag = "External",
    params(
        ("label" = String, Path, description = "Tag label"),
        ("url" = Option<String>, Query, description = "Target URL to inspect"),
        ("id" = Option<String>, Query, description = "Precomputed external link identifier"),
        ("skip" = Option<usize>, Query, description = "Number of taggers to skip for pagination. Defaults to 0"),
        ("limit" = Option<usize>, Query, description = "Number of taggers to return for pagination. Defaults to 40"),
    ),
    responses(
        (status = 200, description = "External taggers", body = TaggersInfoResponse),
        (status = 404, description = "Tags not found"),
        (status = 400, description = "Invalid input"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn external_taggers_handler(
    Path(label): Path<String>,
    Query(query): Query<ExternalTaggersQuery>,
) -> Result<Json<TaggersInfoResponse>> {
    let link_id = resolve_external_id(query.url, query.id)?;
    info!(
        "GET {EXTERNAL_TAGGERS_ROUTE} link_id:{}, label:{}, skip:{:?}, limit:{:?}",
        link_id, label, query.pagination.skip, query.pagination.limit
    );

    match TagExternal::get_taggers_by_id(&link_id, &label, query.pagination, None).await {
        Ok(Some(taggers)) => Ok(Json(TaggersInfoResponse::from(taggers))),
        Ok(None) => Err(Error::TagsNotFound { reach: link_id }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

fn resolve_external_id(url: Option<String>, id: Option<String>) -> Result<String> {
    if let Some(id) = id.filter(|value| !value.is_empty()) {
        return Ok(id);
    }

    let Some(url) = url else {
        return Err(Error::invalid_input(
            "Either `url` or `id` must be provided",
        ));
    };

    ExternalLinkDetails::from_url(&url, 0)
        .map(|details| details.id)
        .map_err(|err| Error::invalid_input(&format!("Invalid external url: {err}")))
}

#[derive(OpenApi)]
#[openapi(
    paths(external_tags_handler, external_taggers_handler),
    components(schemas(TagDetails, TaggersInfoResponse))
)]
pub struct ExternalTagsApiDoc;
