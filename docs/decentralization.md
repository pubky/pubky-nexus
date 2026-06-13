# Nexus and HS Decentralization

These are technical notes describing configuration fields related to Decentralization.

## Indexing 3rd-party Homeservers

### `StackConfig` / `external_hs_pk_blacklist`

List of external HS PKs from which new events are not being indexed, for as long as they are on this list.

This list is consulted when indexing 3rd party HSs. Any existing events from users pointing to one of these HSs are not affected.

This list is also checked when ingesting new users, for example via the Nexus REST API. New users which point to one of these HSs will not be ingested. Any users that already were ingested, who now point to a blacklisted HS, are not affected in the sense that their old data is not deleted; however new events from their new blacklisted HS are not being indexed.

Events that depend on a not-yet-ingested user hosted by a blacklisted HS (a follow of such a user, a tag on them or their posts, a reply or repost referencing their posts) are dropped rather than queued for retry, since the dependency cannot be ingested while the HS is blacklisted. Removing the HS from the list later does not recover these dropped events. Posts that merely mention such a user are still indexed; only the mention relationship is not materialized.
