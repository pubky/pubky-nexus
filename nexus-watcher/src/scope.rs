//! What only the watcher needs of the scope: the moderator pinned into it, which
//! homeservers it reads as a whole feed, and who a seed scope's members are. The scope
//! itself, which the API shares, lives in `nexus_common::models::scope`.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use nexus_common::db::{fetch_key_from_graph, queries};
use nexus_common::models::error::ModelResult;
use nexus_common::models::scope::{scope_from_config, Scope};
use nexus_common::models::user::{PkdnsHomeserverResolver, UserIngestor};
use nexus_common::{ScopeConfig, WatcherConfig};
use pubky_app_specs::PubkyId;

/// A scope that also always includes some users, whatever the setting: the moderator,
/// whose tags take content down, must be read on every Nexus. Pinned users never
/// widen the scope themselves; who they follow doesn't matter.
#[derive(Debug)]
pub struct PinnedScope {
    inner: Arc<dyn Scope>,
    pinned: HashSet<String>,
}

impl PinnedScope {
    pub fn new(inner: Arc<dyn Scope>, pinned: impl IntoIterator<Item = PubkyId>) -> Self {
        Self {
            inner,
            pinned: pinned.into_iter().map(|id| id.to_string()).collect(),
        }
    }
}

#[async_trait::async_trait]
impl Scope for PinnedScope {
    fn config(&self) -> &ScopeConfig {
        self.inner.config()
    }

    async fn is_in_scope(&self, user_id: &str, hs_id: Option<&str>) -> ModelResult<bool> {
        if self.pinned.contains(user_id) {
            return Ok(true);
        }
        self.inner.is_in_scope(user_id, hs_id).await
    }
}

/// The watcher's user ingestor: `config`'s blacklist and scope, with the moderator always
/// in the scope, looking users' homeservers up through `resolver`.
pub fn watcher_ingestor(
    config: &WatcherConfig,
    resolver: Arc<dyn PkdnsHomeserverResolver>,
) -> UserIngestor {
    let scope = PinnedScope::new(
        scope_from_config(&config.stack.scope),
        [config.moderation_id.clone()],
    );
    UserIngestor::from_config(&config.stack, resolver).with_scope(Arc::new(scope))
}

/// Homeservers whose whole `/events` feed is read. Users on any other homeserver are read
/// one by one through their own event stream, if they are in scope.
///
/// For everything, the configured `homeserver`, read as it always was; for a homeserver
/// scope, the listed ones, but none on the HS blacklist; for a seed scope, none, as its
/// members are spread over any homeserver. This is the only place it is decided: the
/// runners take the result.
pub fn full_feed_homeservers(config: &WatcherConfig) -> Vec<PubkyId> {
    match &config.stack.scope {
        ScopeConfig::Everything => vec![config.homeserver.clone()],
        ScopeConfig::Homeservers { homeservers } => {
            let blacklist = &config.stack.net.external_hs_pk_blacklist;
            homeservers
                .iter()
                .filter(|listed| !blacklist.contains(listed))
                .cloned()
                .collect()
        }
        ScopeConfig::Seed { .. } => Vec::new(),
    }
}

/// Set when a follow is indexed or removed, so a seed scope works its members out again
/// at its next check (see the scope maintenance). Kept in memory: a start works them out
/// anyway.
static FOLLOWS_CHANGED: AtomicBool = AtomicBool::new(false);

/// Records that a follow was indexed or removed.
pub(crate) fn follows_changed() {
    FOLLOWS_CHANGED.store(true, Ordering::Release);
}

/// Whether a follow changed since the last call, and clears it: one changed after this
/// asks for the next check.
pub(crate) fn take_follows_changed() -> bool {
    FOLLOWS_CHANGED.swap(false, Ordering::AcqRel)
}

/// A seed scope's members: the seed at hop 0 and everyone it reaches in at most `hops`
/// follows, each at its shortest distance. Worked out from the graph, which holds every
/// follow every member made whatever order they were read in, so the same follows always
/// give the same members. A member whose profile is deleted stays one: its stream, still
/// read, may write a new profile. One query per hop.
pub async fn seed_members(seed: &PubkyId, hops: u8) -> ModelResult<HashMap<String, u8>> {
    let mut members = HashMap::from([(seed.to_string(), 0)]);
    let mut frontier = vec![seed.to_string()];
    for hop in 1..=hops {
        if frontier.is_empty() {
            break;
        }
        let query = queries::get::get_followees_of(&frontier);
        let followees: Vec<String> = fetch_key_from_graph(query, "user_ids")
            .await?
            .unwrap_or_default();
        frontier = followees
            .into_iter()
            .filter(|followee| !members.contains_key(followee))
            .collect();
        for followee in &frontier {
            members.insert(followee.clone(), hop);
        }
    }
    Ok(members)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_common::models::scope::HomeserverListScope;
    use nexus_common::utils::test_utils::random_pubky_id;

    /// Resolves no user's homeserver, without the network.
    struct NoResolver;

    #[async_trait::async_trait]
    impl PkdnsHomeserverResolver for NoResolver {
        async fn resolve_homeserver(
            &self,
            _: &pubky::PublicKey,
        ) -> nexus_common::db::PubkyClientResult<Option<PubkyId>> {
            Ok(None)
        }
    }

    /// Everything reads the configured homeserver whole, as before scopes, even if it is
    /// blacklisted; a homeserver scope reads its list instead, minus blacklisted ones; a
    /// seed scope reads none whole.
    #[test]
    fn the_whole_feeds_come_from_the_scope() {
        let (homeserver, listed, blacklisted) =
            (random_pubky_id(), random_pubky_id(), random_pubky_id());
        let mut config = WatcherConfig {
            homeserver: homeserver.clone(),
            ..WatcherConfig::default()
        };
        config.stack.net.external_hs_pk_blacklist = vec![homeserver.clone(), blacklisted.clone()];

        assert_eq!(full_feed_homeservers(&config), vec![homeserver]);

        config.stack.scope = ScopeConfig::Homeservers {
            homeservers: vec![listed.clone(), blacklisted],
        };
        assert_eq!(full_feed_homeservers(&config), vec![listed]);

        config.stack.scope = ScopeConfig::Seed {
            seed: random_pubky_id(),
            hops: 2,
        };
        assert_eq!(full_feed_homeservers(&config), Vec::<PubkyId>::new());
    }

    /// The moderator is in scope whatever the setting, so its takedowns are read.
    #[tokio::test]
    async fn pinned_users_are_always_in_scope() {
        let (moderator, listed, unlisted) =
            (random_pubky_id(), random_pubky_id(), random_pubky_id());
        let homeservers: Arc<dyn Scope> = Arc::new(HomeserverListScope::new(&[listed]));
        let scope = PinnedScope::new(homeservers, [moderator.clone()]);

        assert!(scope
            .is_in_scope(&moderator, Some(&unlisted))
            .await
            .unwrap());
        assert!(!scope.is_in_scope("someone", Some(&unlisted)).await.unwrap());
    }

    /// The watcher's ingestor keeps the moderator in scope whatever the scope: on a
    /// homeserver the list doesn't hold, the moderator is in, and anyone else is out.
    #[tokio::test]
    async fn the_watchers_ingestor_keeps_the_moderator_in_scope() {
        let (listed, unlisted) = (random_pubky_id(), random_pubky_id());
        let mut config = WatcherConfig::default();
        config.stack.scope = ScopeConfig::Homeservers {
            homeservers: vec![listed],
        };
        let scope = watcher_ingestor(&config, Arc::new(NoResolver))
            .scope()
            .clone();

        let moderator = config.moderation_id.to_string();
        assert!(scope
            .is_in_scope(&moderator, Some(&unlisted))
            .await
            .unwrap());
        let someone = random_pubky_id().to_string();
        assert!(!scope.is_in_scope(&someone, Some(&unlisted)).await.unwrap());
    }
}
