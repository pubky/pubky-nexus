use crate::models::{
    BoundedLimit, BoundedPagination, BoundedSkip, PostSearchQuery, PubkyAppPostKind, PubkyId,
    TagLabel,
};
use crate::routes::v0::endpoints::{SEARCH_POSTS_BY_CONTENT_ROUTE, SEARCH_POSTS_BY_TAG_ROUTE};
use crate::routes::{Path, Query};
use crate::{Error, Result};
use axum::Json;
use nexus_common::db::kv::AuthorFilter;
use nexus_common::models::follow::reach::{reach_authors, reach_contains, ReachAuthors};
use nexus_common::models::post::search::{
    PostsByContentSearch, PostsByTagSearch, MAX_REACH_AUTHORS_FT,
};
use nexus_common::types::{StreamReach, StreamSorting};
use opentelemetry::metrics::{Histogram, Meter};
use opentelemetry::{global, KeyValue};
use serde::Deserialize;
use std::sync::LazyLock;
use tracing::debug;
use utoipa::OpenApi;

#[derive(Deserialize)]
pub struct SearchPostsQuery {
    pub sorting: Option<StreamSorting>,
    pub user_id: Option<PubkyId>,
    pub reach: Option<StreamReach>,
    #[serde(flatten)]
    pub pagination: BoundedPagination<10_000, 20, 200>,
    pub start: Option<f64>,
    pub end: Option<f64>,
}

#[utoipa::path(
    get,
    path = SEARCH_POSTS_BY_TAG_ROUTE,
    description = "Search Posts by Tag. With `user_id` and `reach`, only parent posts authored by users in that reach are returned (the observer's own posts excluded), and the `total_engagement` score counts taggers, replies and reposts but not mentions. `start`/`end` cursors are not interchangeable between the reach and non-reach modes",
    tag = "Search",
    params(
        ("tag" = TagLabel, Path, description = "Tag name"),
        ("sorting" = Option<StreamSorting>, Query, description = "StreamSorting method"),
        ("user_id" = Option<PubkyId>, Query, description = "User ID to base reach on. Must be provided together with reach"),
        ("reach" = Option<StreamReach>, Query, example = "wot_2", description = "Reach type: `followers` | `following` | `friends` | `wot` | `wot_1`..`wot_3`. To apply that, user_id is required. Bare `wot` defaults to depth 2."),
        ("start" = Option<f64>, Query, description = "The start of the stream timeframe (score cursor for `total_engagement`). Posts with a score greater than this value will be excluded from the results"),
        ("end" = Option<f64>, Query, description = "The end of the stream timeframe (score cursor for `total_engagement`). Posts with a score less than this value will be excluded from the results"),
        ("skip" = Option<BoundedSkip<10_000>>, Query, description = "Skip N results (max 10000)"),
        ("limit" = Option<BoundedLimit<20, 200>>, Query, description = "Limit the number of results (1–200, default 20)")
    ),
    responses(
        (status = 200, description = "Search results", body = Vec<PostsByTagSearch>),
        (status = 400, description = "Invalid parameters"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn search_posts_by_tag_handler(
    Path(tag): Path<TagLabel>,
    Query(query): Query<SearchPostsQuery>,
) -> Result<Json<Vec<PostsByTagSearch>>> {
    let sorting = query.sorting;

    debug!(
        "GET {SEARCH_POSTS_BY_TAG_ROUTE} tag:{}, sort_by: {:?}, user_id: {:?}, reach: {:?}, start: {:?}, end: {:?}, skip: {}, limit: {}",
        tag, sorting, query.user_id, query.reach, query.start, query.end,
        query.pagination.skip_value(), query.pagination.limit_value()
    );

    if query.user_id.is_some() ^ query.reach.is_some() {
        return Err(Error::invalid_input(
            "user_id and reach should be both provided together",
        ));
    }

    let pagination = query.pagination.to_pagination(query.start, query.end);

    if let (Some(user_id), Some(reach)) = (query.user_id, query.reach) {
        let posts =
            PostsByTagSearch::get_by_label_with_reach(&tag, sorting, &user_id, reach, pagination)
                .await?;
        return Ok(Json(posts));
    }

    match PostsByTagSearch::get_by_label(&tag, sorting, pagination).await? {
        Some(posts_list) => Ok(Json(posts_list)),
        None => Ok(Json(vec![])),
    }
}

const METER_NAME: &str = "search.posts.by_content";

/// How many authors the reaches a full-text content search resolves hold, and
/// how often `MAX_REACH_AUTHORS_FT` cut one short. The instrument is a no-op
/// when no `SdkMeterProvider` is registered (i.e. when OTLP is not configured),
/// so there is zero overhead in that case.
struct ContentSearchMetrics {
    reach_authors: Histogram<u64>,
}

impl ContentSearchMetrics {
    fn new(meter: &Meter) -> Self {
        Self {
            reach_authors: meter
                .u64_histogram("search.posts.by_content.reach.authors")
                .with_description(
                    "Authors a content search's reach resolved to, by reach/depth/met_limit",
                )
                .with_unit("{user}")
                .build(),
        }
    }

    /// `reach`/`depth` are the attributes the reach graph queries already
    /// carry, so a reach can be followed across both. `met_limit` splits off the
    /// searches that only saw part of the reach.
    fn record_reach_resolution(&self, reach: &StreamReach, authors: usize, met_limit: bool) {
        let (name, depth) = reach.telemetry_dimensions();
        let mut attrs = vec![
            KeyValue::new("reach", name),
            KeyValue::new("met_limit", met_limit),
        ];
        if let Some(depth) = depth {
            attrs.push(KeyValue::new("depth", i64::from(depth)));
        }
        self.reach_authors.record(authors as u64, &attrs);
    }
}

static METRICS: LazyLock<ContentSearchMetrics> =
    LazyLock::new(|| ContentSearchMetrics::new(&global::meter(METER_NAME)));

/// Which posts a content search runs over, decided from the query alone so the
/// handler only has to execute it.
#[derive(Debug, PartialEq)]
enum Scope<'a> {
    Unscoped,
    Author(&'a PubkyId),
    /// author and reach intersect: the author's posts, if in reach
    AuthorInReach {
        observer: &'a PubkyId,
        reach: &'a StreamReach,
        author: &'a PubkyId,
    },
    Reach {
        observer: &'a PubkyId,
        reach: &'a StreamReach,
    },
}

fn resolve_scope<'a>(
    author: Option<&'a PubkyId>,
    user_id: Option<&'a PubkyId>,
    reach: Option<&'a StreamReach>,
) -> Result<Scope<'a>> {
    match (author, user_id, reach) {
        (Some(author), Some(observer), Some(reach)) => Ok(Scope::AuthorInReach {
            observer,
            reach,
            author,
        }),
        (None, Some(observer), Some(reach)) => Ok(Scope::Reach { observer, reach }),
        (Some(author), None, None) => Ok(Scope::Author(author)),
        (None, None, None) => Ok(Scope::Unscoped),
        (_, Some(_), None) | (_, None, Some(_)) => Err(Error::invalid_input(
            "user_id and reach should be both provided together",
        )),
    }
}

/// The authors a `Scope::Reach` search runs over, recording how many the
/// reach resolved to and whether it was trimmed.
fn resolved_reach_authors(
    metrics: &ContentSearchMetrics,
    reach: &StreamReach,
    authors: ReachAuthors,
) -> Vec<PubkyId> {
    metrics.record_reach_resolution(reach, authors.author_ids.len(), authors.met_limit);
    authors.author_ids
}

/// The filter a `Scope::AuthorInReach` search runs with, or `None` when the
/// author is out of reach and the search has no results. Records nothing on
/// `metrics`: a membership check resolves no reach, so there is no size to
/// report.
fn author_in_reach_filter<'a>(
    _metrics: &ContentSearchMetrics,
    author: &'a PubkyId,
    in_reach: bool,
) -> Option<AuthorFilter<'a>> {
    in_reach.then_some(AuthorFilter::One(author))
}

#[derive(Deserialize)]
pub struct SearchPostsByContentQuery {
    pub q: PostSearchQuery,
    pub author: Option<PubkyId>,
    pub kind: Option<PubkyAppPostKind>,
    pub user_id: Option<PubkyId>,
    pub reach: Option<StreamReach>,
    #[serde(flatten)]
    pub pagination: BoundedPagination<1000, 20, 100>,
}

#[utoipa::path(
    get,
    path = SEARCH_POSTS_BY_CONTENT_ROUTE,
    description = "Full-text search over post content",
    tag = "Search",
    params(
        ("q" = PostSearchQuery, Query, description = "Search query (2–30 characters, up to 4 terms)"),
        ("author" = Option<PubkyId>, Query, description = "Optional author Pubky ID to scope results"),
        ("kind" = Option<PubkyAppPostKind>, Query, description = "Optional post kind to filter by: short, long, image, video, link, file, collection"),
        ("user_id" = Option<PubkyId>, Query, description = "User ID to base reach on. Must be provided together with reach"),
        ("reach" = Option<StreamReach>, Query, example = "following", description = "Reach type: `followers` | `following` | `friends` | `wot` | `wot_1`..`wot_3`. Scopes results to posts authored by users in that reach, never by user_id itself. To apply that, user_id is required. Bare `wot` defaults to depth 2. Combined with `author`, results are that author's posts if the author is in reach, and empty otherwise"),
        ("skip" = Option<BoundedSkip<1000>>, Query, description = "Skip N results (max 1000)"),
        ("limit" = Option<BoundedLimit<20, 100>>, Query, description = "Limit the number of results (1–100, default 20)")
    ),
    responses(
        (status = 200, description = "Search results ordered by relevance score", body = Vec<PostsByContentSearch>),
        (status = 400, description = "Invalid query or limit parameter"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn search_posts_by_content_handler(
    Query(query): Query<SearchPostsByContentQuery>,
) -> Result<Json<Vec<PostsByContentSearch>>> {
    let skip = query.pagination.skip_value();
    let limit = query.pagination.limit_value();

    debug!(
        "GET {SEARCH_POSTS_BY_CONTENT_ROUTE} q:{}, author:{:?}, kind:{:?}, user_id:{:?}, reach:{:?}, skip:{skip}, limit:{limit}",
        query.q, query.author, query.kind, query.user_id, query.reach
    );

    let scope = resolve_scope(
        query.author.as_ref(),
        query.user_id.as_ref(),
        query.reach.as_ref(),
    )?;

    let kind_str = query.kind.as_ref().map(|k| k.to_string());

    let author_ids;
    let author = match scope {
        Scope::Unscoped => None,
        Scope::Author(author) => Some(AuthorFilter::One(author)),
        Scope::AuthorInReach {
            observer,
            reach,
            author,
        } => {
            let in_reach = reach_contains(observer, reach, author).await?;
            let Some(filter) = author_in_reach_filter(&METRICS, author, in_reach) else {
                return Ok(Json(vec![]));
            };
            Some(filter)
        }
        Scope::Reach { observer, reach } => {
            // Over the cap, the most prolific authors are kept; how often
            // that happens is in the `met_limit` attribute of the metric
            let authors = reach_authors(observer, reach, MAX_REACH_AUTHORS_FT).await?;
            author_ids = resolved_reach_authors(&METRICS, reach, authors);
            Some(AuthorFilter::AnyOf(&author_ids))
        }
    };

    let results =
        PostsByContentSearch::search(query.q.as_str(), author, kind_str.as_deref(), skip, limit)
            .await?;
    Ok(Json(results))
}

#[derive(OpenApi)]
#[openapi(
    paths(search_posts_by_tag_handler, search_posts_by_content_handler),
    components(schemas(
        PostsByTagSearch,
        PostsByContentSearch,
        PostSearchQuery,
        PubkyAppPostKind,
        StreamReach
    ))
)]
pub struct SearchPostsApiDocs;

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_common::types::WotDepth;
    use opentelemetry::metrics::MeterProvider;
    use opentelemetry_sdk::metrics::data::{
        AggregatedMetrics, Metric, MetricData, ResourceMetrics,
    };
    use opentelemetry_sdk::metrics::{InMemoryMetricExporter, PeriodicReader, SdkMeterProvider};

    fn parse_query(
        s: &str,
    ) -> std::result::Result<SearchPostsByContentQuery, serde_urlencoded::de::Error> {
        serde_urlencoded::from_str(s)
    }

    #[test]
    fn author_missing_parses_unscoped() {
        let q = parse_query("q=bitcoin").expect("valid query must parse");
        assert!(q.author.is_none());
        assert!(q.kind.is_none());
    }

    #[test]
    fn author_invalid_format_rejected() {
        assert!(parse_query("q=bitcoin&author=not-a-pubky").is_err());
    }

    #[test]
    fn reach_parses_with_user_id() {
        let q = parse_query(
            "q=bitcoin&user_id=wnhrmj3b1tt3n6fr7fhedgak4q11e9i1uxm4dmiactgeobyu9wpy&reach=wot_3",
        )
        .expect("valid reach must parse");
        assert!(q.user_id.is_some());
        assert!(matches!(q.reach, Some(StreamReach::Wot(depth)) if depth.get() == 3));
    }

    #[test]
    fn reach_invalid_rejected() {
        assert!(parse_query("q=bitcoin&reach=wot_4").is_err());
        assert!(parse_query("q=bitcoin&reach=everyone").is_err());
    }

    fn pubky_id(id: &str) -> PubkyId {
        PubkyId::try_from(id).expect("valid pubky id")
    }

    fn observer() -> PubkyId {
        pubky_id("wnhrmj3b1tt3n6fr7fhedgak4q11e9i1uxm4dmiactgeobyu9wpy")
    }

    fn author() -> PubkyId {
        pubky_id("y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy")
    }

    #[test]
    fn scope_without_author_or_reach_is_unscoped() {
        assert_eq!(resolve_scope(None, None, None).unwrap(), Scope::Unscoped);
    }

    #[test]
    fn scope_with_author_alone_is_that_author() {
        let author = author();
        assert_eq!(
            resolve_scope(Some(&author), None, None).unwrap(),
            Scope::Author(&author)
        );
    }

    #[test]
    fn scope_with_one_of_user_id_and_reach_is_rejected() {
        let (observer, author) = (observer(), author());
        let reach = StreamReach::Following;
        for author in [None, Some(&author)] {
            for (user_id, reach) in [(Some(&observer), None), (None, Some(&reach))] {
                let err = resolve_scope(author, user_id, reach)
                    .expect_err("user_id and reach must come together");
                assert!(
                    matches!(err, Error::InvalidInput { .. }),
                    "expected InvalidInput, got {err:?}"
                );
            }
        }
    }

    /// Never a plain `Author`, which would skip the membership check.
    #[test]
    fn scope_with_author_and_reach_checks_the_author_against_the_reach() {
        let (observer, author) = (observer(), author());
        let reach = StreamReach::Friends;
        assert_eq!(
            resolve_scope(Some(&author), Some(&observer), Some(&reach)).unwrap(),
            Scope::AuthorInReach {
                observer: &observer,
                reach: &reach,
                author: &author,
            }
        );
    }

    #[test]
    fn scope_with_reach_alone_is_the_whole_reach() {
        let observer = observer();
        let reach = StreamReach::Wot(WotDepth::new(3).expect("valid depth"));
        assert_eq!(
            resolve_scope(None, Some(&observer), Some(&reach)).unwrap(),
            Scope::Reach {
                observer: &observer,
                reach: &reach,
            }
        );
    }

    #[test]
    fn kind_missing_parses_unscoped() {
        let q = parse_query("q=bitcoin").expect("valid query must parse");
        assert!(q.kind.is_none());
    }

    #[test]
    fn kind_valid_short_accepted() {
        let q = parse_query("q=bitcoin&kind=short").expect("valid kind must parse");
        assert_eq!(q.kind, Some(PubkyAppPostKind::Short));
    }

    #[test]
    fn kind_unknown_parses_as_unknown() {
        let q =
            parse_query("q=bitcoin&kind=not-a-kind").expect("lenient kind parsing must not error");
        assert_eq!(q.kind, Some(PubkyAppPostKind::Unknown));
    }

    /// `attr=value` pairs of a data point, sorted, so the assertions don't
    /// depend on the order the SDK keeps them in.
    fn attrs(point: impl Iterator<Item = KeyValue>) -> Vec<String> {
        let mut attrs: Vec<String> = point.map(|kv| format!("{}={}", kv.key, kv.value)).collect();
        attrs.sort();
        attrs
    }

    /// Every point of `name` as `(attributes, sum)`, sorted by sum.
    fn points(exported: &[&Metric], name: &str) -> Vec<(Vec<String>, u64)> {
        let data = exported
            .iter()
            .find(|m| m.name() == name)
            .unwrap_or_else(|| panic!("{name} must be exported"))
            .data();
        let mut points: Vec<_> = match data {
            AggregatedMetrics::U64(MetricData::Histogram(h)) => h
                .data_points()
                .map(|p| (attrs(p.attributes().cloned()), p.sum()))
                .collect(),
            other => panic!("unexpected aggregation for {name}: {other:?}"),
        };
        points.sort_by_key(|(_, sum)| *sum);
        points
    }

    /// Metrics that export to memory, so a test reads back what was recorded.
    struct ExportedMetrics {
        exporter: InMemoryMetricExporter,
        provider: SdkMeterProvider,
        metrics: ContentSearchMetrics,
    }

    impl ExportedMetrics {
        fn new() -> Self {
            let exporter = InMemoryMetricExporter::default();
            let provider = SdkMeterProvider::builder()
                .with_reader(PeriodicReader::builder(exporter.clone()).build())
                .build();
            let metrics = ContentSearchMetrics::new(&provider.meter(METER_NAME));
            Self {
                exporter,
                provider,
                metrics,
            }
        }

        fn collect(&self) -> Vec<ResourceMetrics> {
            self.provider.force_flush().expect("flush must succeed");
            self.exporter
                .get_finished_metrics()
                .expect("metrics collected")
        }
    }

    fn metrics_of(collected: &[ResourceMetrics]) -> Vec<&Metric> {
        collected
            .iter()
            .flat_map(|rm| rm.scope_metrics())
            .flat_map(|sm| sm.metrics())
            .collect()
    }

    fn reach_of(author_ids: &[&str], met_limit: bool) -> ReachAuthors {
        ReachAuthors {
            author_ids: author_ids.iter().map(|id| pubky_id(id)).collect(),
            met_limit,
        }
    }

    /// The counts are far from the cap and the smaller reach is the trimmed
    /// one, so neither the count nor `met_limit` can come from anywhere but
    /// the resolved reach.
    #[test]
    fn reach_search_records_the_resolved_authors_and_returns_them() {
        let exported = ExportedMetrics::new();
        let wot_2 = StreamReach::Wot(WotDepth::new(2).expect("valid depth"));
        let trimmed = reach_of(
            &[
                "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy",
                "wnhrmj3b1tt3n6fr7fhedgak4q11e9i1uxm4dmiactgeobyu9wpy",
            ],
            true,
        );
        let whole = reach_of(
            &[
                "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro",
                "58jc5bujzoj35g55pqjo6ykfdu9t156j8cxkh5ubdwgsnch1qagy",
                "5f4e8eoogmkhqeyo5ijdix3ma6rw9byj8m36yrjp78pnxxc379to",
            ],
            false,
        );

        let trimmed_ids =
            resolved_reach_authors(&exported.metrics, &StreamReach::Following, trimmed.clone());
        let whole_ids = resolved_reach_authors(&exported.metrics, &wot_2, whole.clone());

        assert_eq!(trimmed_ids, trimmed.author_ids);
        assert_eq!(whole_ids, whole.author_ids);

        let collected = exported.collect();
        assert_eq!(
            points(
                &metrics_of(&collected),
                "search.posts.by_content.reach.authors"
            ),
            vec![
                (
                    vec!["met_limit=true".to_string(), "reach=following".to_string()],
                    2
                ),
                (
                    vec![
                        "depth=2".to_string(),
                        "met_limit=false".to_string(),
                        "reach=wot".to_string()
                    ],
                    3
                ),
            ]
        );
    }

    /// `author` + `reach` is a membership check: no reach is resolved, so
    /// there is no size to record, whether the author is in it or not.
    #[test]
    fn author_in_reach_search_records_nothing() {
        let exported = ExportedMetrics::new();
        let author = author();

        let in_reach = author_in_reach_filter(&exported.metrics, &author, true);
        let out_of_reach = author_in_reach_filter(&exported.metrics, &author, false);

        assert!(matches!(in_reach, Some(AuthorFilter::One(id)) if *id == author));
        assert!(out_of_reach.is_none());

        let collected = exported.collect();
        let recorded = metrics_of(&collected)
            .into_iter()
            .filter(|m| m.name() == "search.posts.by_content.reach.authors")
            .count();
        assert_eq!(recorded, 0, "a membership check must not record a reach");
    }

    /// Asserts on the exported points, not on the calls: an instrument renamed
    /// or an attribute dropped is what breaks the dashboards.
    #[test]
    fn records_reach_size_and_whether_the_limit_was_met() {
        let exported = ExportedMetrics::new();
        let wot_3 = StreamReach::Wot(WotDepth::new(3).expect("valid depth"));

        exported
            .metrics
            .record_reach_resolution(&StreamReach::Following, 7, false);
        exported
            .metrics
            .record_reach_resolution(&wot_3, MAX_REACH_AUTHORS_FT, true);

        let collected = exported.collect();

        // met_limit splits the searches that missed part of the reach off the
        // same instrument, so no second one is needed to tell them apart
        assert_eq!(
            points(
                &metrics_of(&collected),
                "search.posts.by_content.reach.authors"
            ),
            vec![
                (
                    vec!["met_limit=false".to_string(), "reach=following".to_string()],
                    7
                ),
                (
                    vec![
                        "depth=3".to_string(),
                        "met_limit=true".to_string(),
                        "reach=wot".to_string()
                    ],
                    MAX_REACH_AUTHORS_FT as u64
                ),
            ]
        );
    }
}
