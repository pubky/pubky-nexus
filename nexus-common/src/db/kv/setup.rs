use crate::db::kv::RedisResult;
use crate::models::post::create_post_content_index;
use tracing::info;

/// Ensure the Redis cache has the required RediSearch indexes.
///
/// This is the Redis counterpart of [`crate::db::setup::setup_graph`]: the single
/// declaration of the search schema that must exist in every environment. Future
/// RediSearch indexes get added here.
///
/// Unlike `setup_graph`, this is deliberately **not** wrapped in a `OnceCell`.
/// Neo4j's `MATCH (n) DETACH DELETE n` preserves constraints and indexes, so the
/// graph DDL only ever needs to run once per process. `FLUSHDB` destroys the FT
/// index along with the keys, so `setup_cache` must be re-runnable after every
/// flush (see [`crate::db::kv::clear_redis`]). A `OnceCell` here would leave any
/// path that flushes Redis mid-process (e.g. the reindex bench) without a search
/// index. Every index created here is idempotent, so repeated calls are cheap.
///
/// Safe to call during connector init: the key prefix is derived from the type
/// name and needs no live connection beyond the one used to issue `FT.CREATE`.
///
/// Nexus requires a Redis that ships the query engine (`docker-compose` pins
/// `redis:8.0.6-alpine`). A failed `FT.CREATE` is fatal at connector init, so a
/// Redis without it stops every binary from starting instead of degrading
/// search. This is deliberate: a missing index is a misconfigured environment,
/// not a runtime condition to route around.
pub async fn setup_cache() -> RedisResult<()> {
    create_post_content_index().await?;

    info!("Redis search indexes have been applied successfully");

    Ok(())
}
