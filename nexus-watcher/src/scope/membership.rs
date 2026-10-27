//! Moving users in and out of the scope, which only the watcher does: the user ingestor
//! only decides, for a user met for the first time, whether it is a member or an outsider.

use deadpool_redis::redis;
use nexus_common::db::kv::RedisError;
use nexus_common::db::{exec_single_row, fetch_key_from_graph, get_redis_conn, queries};
use nexus_common::models::error::{ModelError, ModelResult};
use nexus_common::models::traits::Collection;
use nexus_common::models::user::catch_up::{caught_up, start_catch_up};
use nexus_common::models::user::outsider::admit_outsider;
use nexus_common::models::user::{
    set_user_homeserver, UserDetails, UserHsCursor, UserIngestor, SCOPE_STATE_LEFT,
};
use nexus_common::ScopeConfig;
use pubky_app_specs::PubkyId;

/// Marks a user outside the scope whose homeserver was rechecked lately.
const RECHECKED_PREFIX: &str = "Scope:Rechecked:";
const RECHECK_EVERY_S: u64 = 15 * 60;

/// Brings users into the scope and takes them out of it, looking their homeservers up and
/// deciding by the scope of the ingestor it is given.
pub struct ScopeMembership<'a> {
    ingestor: &'a UserIngestor,
}

impl<'a> ScopeMembership<'a> {
    pub fn of(ingestor: &'a UserIngestor) -> Self {
        Self { ingestor }
    }

    /// Makes sure a user that must be in the scope (the moderator) is known and polled.
    ///
    /// If its HS can't be resolved right now, the bare node is still created and let into
    /// the scope: the HS resolver binds it on a later run, and polling starts from there.
    /// A node kept as an outsider (the API ingests with the plain scope, which doesn't pin
    /// the moderator) is brought back into the scope.
    pub async fn ensure_in_scope(&self, user_id: &PubkyId) -> ModelResult<()> {
        self.ingestor.maybe_ingest_user(user_id).await?;
        if UserDetails::get_by_id(user_id).await?.is_none() {
            UserDetails::from_pubky(user_id.clone())
                .put_to_graph()
                .await?;
        }
        self.let_in(user_id).await
    }

    /// Lets a user kept outside the scope in, whatever its homeserver: [`Self::promote`]s
    /// it, or, with no published HS, clears its state so the HS resolver binds it later.
    pub async fn let_in(&self, user_id: &PubkyId) -> ModelResult<()> {
        if self.promote(user_id).await? {
            return Ok(());
        }
        // No published HS: in scope without a binding, for the HS resolver to bind.
        clear_scope_state(user_id).await?;
        tracing::info!("Let {user_id} into the scope; its HS is bound once published");
        Ok(())
    }

    /// Brings a user kept outside the scope (an outsider, or a member that left) back in:
    /// binds it to its HS with a cursor at 0, so it is read from the start of its
    /// stream. The rankings, rebuilt from the graph, take it in from their next rebuild.
    ///
    /// Returns whether the user is in scope now: `true` if it was promoted or already in,
    /// `false` if it is unknown or has no published HS, so it couldn't be bound.
    pub async fn promote(&self, user_id: &PubkyId) -> ModelResult<bool> {
        let query = queries::get::get_user_scope_state(user_id);
        let scope_state: Option<Option<String>> =
            fetch_key_from_graph(query, "scope_state").await?;
        match scope_state {
            None => return Ok(false),
            Some(None) => return Ok(true),
            Some(Some(_)) => {}
        }

        let Some(hs_id) = self.ingestor.ensure_hs_not_blacklisted(user_id).await? else {
            tracing::warn!("Not promoting {user_id}: no published HS");
            return Ok(false);
        };

        // The graph's state is cleared last: if anything before fails, the user is still
        // out of scope there and the next promotion retries the whole of it.
        set_user_homeserver(user_id, &hs_id).await?;
        // Read from the start: its records were skipped while it was out, also in the
        // feeds of the homeservers read whole, and a poll still running when it left may
        // have written a cursor back.
        UserHsCursor::reset(user_id, &hs_id).await?;
        start_catch_up(user_id, &hs_id).await?;
        clear_scope_state(user_id).await?;
        tracing::info!("Brought {user_id} into the scope");
        Ok(true)
    }

    /// An event from a listed homeserver's feed by an outsider or a former member means it
    /// may have moved onto a listed homeserver. Checked at most once per 15 minutes per
    /// user; promote reads its stream from the start, so skipped events come back.
    ///
    /// Returns whether the user is in scope now. A failed lookup is logged, not returned.
    pub async fn admit_if_moved_in(&self, user_id: &PubkyId) -> ModelResult<bool> {
        // Everything has no outsiders.
        if !matches!(
            self.ingestor.scope().config(),
            ScopeConfig::Homeservers { .. }
        ) {
            return Ok(false);
        }

        let mut redis_conn = get_redis_conn().await?;
        let first: Option<String> = redis::cmd("SET")
            .arg(format!("{RECHECKED_PREFIX}{user_id}"))
            .arg(1)
            .arg("NX")
            .arg("EX")
            .arg(RECHECK_EVERY_S)
            .query_async(&mut redis_conn)
            .await
            .map_err(RedisError::from)?;
        if first.is_none() {
            return Ok(false);
        }

        match self.ingestor.ensure_hs_not_blacklisted(user_id).await {
            Ok(Some(hs_id))
                if self
                    .ingestor
                    .scope()
                    .is_in_scope(user_id, Some(&hs_id))
                    .await? =>
            {
                self.promote(user_id).await
            }
            Ok(_) | Err(ModelError::HsBlacklisted { .. }) => Ok(false),
            Err(e) => {
                tracing::warn!("Could not recheck the homeserver of {user_id}: {e}");
                Ok(false)
            }
        }
    }

    /// Takes a member that left the scope out of it: marks it `left`, unbinds it from its
    /// HS and forgets its cursor, so it is no longer polled. Its content stays; if it
    /// comes back, it is promoted and read from the start of its stream again.
    pub async fn demote(&self, user_id: &str) -> ModelResult<()> {
        exec_single_row(queries::put::unbind_user_from_scope(
            user_id,
            SCOPE_STATE_LEFT,
        ))
        .await?;
        UserHsCursor::remove_all(user_id).await?;
        // A catch-up still running reads users regardless of their binding.
        caught_up(user_id).await?;
        tracing::info!("{user_id} left the scope");
        Ok(())
    }
}

/// Clears the user's `scope_state`: an outsider is admitted (its tags ranked, see
/// [`admit_outsider`]); a member that left was never kept out of anything, so its state is
/// just cleared.
async fn clear_scope_state(user_id: &str) -> ModelResult<()> {
    if !admit_outsider(user_id).await? {
        exec_single_row(queries::put::set_user_scope_state(user_id, None)).await?;
    }
    Ok(())
}
