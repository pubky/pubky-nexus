# Nexus and HS Decentralization

These are technical notes describing configuration fields related to Decentralization.

## Indexing 3rd-party Homeservers

### `StackConfig` / `external_hs_pk_blacklist`

List of external HS PKs from which new events are not being indexed, for as long as they are on this list.

This list is consulted when indexing 3rd party HSs. Any existing events from users poining to one of these HSs are not affected.

This list is also checked when ingesting new users, for example via the Nexus REST API. New users which point to one of these HSs will not be ingested. Any users that already were ingested, who now point to a blacklisted HS, are not affected in the sense that theyir old data is not deleted; however new events from their new blacklisted HS are not being indexed.
