use crate::service::utils::{setup, MockHomeserverClient};
use anyhow::Result;
use nexus_common::db::graph::Query;
use nexus_common::db::{exec_single_row, fetch_key_from_graph, queries};
use nexus_common::models::homeserver::Homeserver;
use nexus_common::models::user::outsider::create_outsider;
use nexus_common::models::user::{is_outsider, UserDetails};
use nexus_common::utils::test_utils::random_pubky_id;
use nexus_common::{ScopeConfig, WatcherConfig};
use nexus_watcher::scope::watcher_ingestor;
use nexus_watcher::service::prepare_scope;
use nexus_watcher::service::scope_maintenance::ScopeMaintainer;
use std::sync::Arc;

/// The start of a homeserver scope persists every listed homeserver, so each has the
/// cursor its feed is read from, and brings a moderator kept as an outsider back into the
/// scope, so its takedowns are read.
#[tokio_shared_rt::test(shared)]
async fn test_prepare_scope_persists_the_listed_homeservers_and_lets_the_moderator_in() -> Result<()>
{
    setup().await?;
    let homeservers = vec![random_pubky_id(), random_pubky_id()];
    let moderator = random_pubky_id();
    let config = WatcherConfig {
        moderation_id: moderator.clone(),
        stack: nexus_common::StackConfig {
            scope: ScopeConfig::Homeservers {
                homeservers: homeservers.clone(),
            },
            ..Default::default()
        },
        ..Default::default()
    };
    // Kept as an outsider, as the API, which doesn't pin the moderator, would keep it.
    create_outsider(&UserDetails::from_pubky(moderator.clone())).await?;

    // The mock publishes no homeserver for anyone: the moderator is let in unbound.
    let client: Arc<MockHomeserverClient> = Arc::new(MockHomeserverClient::default());
    // Limited to the moderator: the graph is shared with the other tests.
    let maintainer = ScopeMaintainer::new(
        &config,
        watcher_ingestor(&config, client.clone()),
        Some(vec![moderator.to_string()]),
    );
    let prepared = prepare_scope(&config, client, &maintainer).await;

    let persisted = [
        Homeserver::get_by_id(homeservers[0].clone()).await?,
        Homeserver::get_by_id(homeservers[1].clone()).await?,
    ];
    let query = queries::get::get_user_scope_state(&moderator);
    let scope_state: Option<Option<String>> = fetch_key_from_graph(query, "scope_state").await?;
    let moderator_is_outsider = is_outsider(&moderator).await?;

    let delete = Query::new(
        "test_delete_moderator",
        "MATCH (u:User {id: $id}) DETACH DELETE u",
    )
    .param("id", moderator.to_string());
    exec_single_row(delete).await?;

    prepared.map_err(|e| anyhow::anyhow!("{e}"))?;
    assert!(
        persisted.iter().all(Option::is_some),
        "every listed homeserver has a cursor to be read from"
    );
    assert_eq!(scope_state.flatten(), None, "the moderator is in scope");
    assert!(!moderator_is_outsider, "and not an outsider in Redis");
    Ok(())
}
