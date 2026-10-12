use std::collections::HashSet;

use pubky_app_specs::PubkyId;
use serde::Deserialize;
use utoipa::ToSchema;

use super::bounded_vec;

/// Most authors `GET /v0/events?users=` accepts.
pub const MAX_EVENT_USERS_QUERY: usize = 100;
/// Most authors `POST /v0/events/by_users` accepts.
pub const MAX_EVENT_USERS_BODY: usize = 1_000;

/// Comma-separated Pubky ids for `GET /v0/events?users=` (1–100).
#[derive(Debug, ToSchema)]
#[schema(value_type = String, example = "ep441mndnsjeesenwz78r9paepm6e4kqm4ggiyy9uzpoe43eu9ny,f5tcy5gtgzshipr6pag6cn9uski3s8tjare7wd3n7enmyokgjk1o")]
pub struct EventUsersQuery(pub HashSet<String>);

/// JSON array of Pubky ids for `POST /v0/events/by_users` (1–1,000).
#[derive(Debug, ToSchema)]
#[schema(value_type = Vec<String>)]
pub struct EventUsers(pub HashSet<String>);

/// A `PubkyId` parsed from one token of a comma-separated list.
struct CsvPubkyId(PubkyId);

impl TryFrom<String> for CsvPubkyId {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        PubkyId::try_from(value.as_str())
            .map(Self)
            .map_err(|e| format!("Invalid PubkyId: {e}"))
    }
}

impl<'de> Deserialize<'de> for EventUsersQuery {
    fn deserialize<D: serde::de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        bounded_vec::deserialize_csv::<CsvPubkyId, D, 1, MAX_EVENT_USERS_QUERY>(d)
            .map(|ids| Self(ids.into_iter().map(|id| id.0.to_string()).collect()))
    }
}

impl<'de> Deserialize<'de> for EventUsers {
    fn deserialize<D: serde::de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        bounded_vec::deserialize_json_array::<PubkyId, D, 1, MAX_EVENT_USERS_BODY>(d)
            .map(|ids| Self(ids.iter().map(PubkyId::to_string).collect()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALICE: &str = "ep441mndnsjeesenwz78r9paepm6e4kqm4ggiyy9uzpoe43eu9ny";
    const BOB: &str = "f5tcy5gtgzshipr6pag6cn9uski3s8tjare7wd3n7enmyokgjk1o";

    #[derive(Deserialize)]
    struct Query {
        users: EventUsersQuery,
    }

    fn query(users: &str) -> Result<Query, serde_urlencoded::de::Error> {
        serde_urlencoded::from_str(&format!("users={users}"))
    }

    #[test]
    fn query_parses_comma_separated_ids() {
        let parsed = query(&format!("{ALICE},{BOB}")).unwrap();
        assert_eq!(
            parsed.users.0,
            HashSet::from([ALICE.to_string(), BOB.to_string()])
        );
    }

    #[test]
    fn query_rejects_invalid_empty_and_too_many() {
        assert!(query("not-a-key").is_err());
        assert!(query("").is_err());
        let too_many = vec![ALICE; MAX_EVENT_USERS_QUERY + 1].join(",");
        assert!(query(&too_many).is_err());
        let max = vec![ALICE; MAX_EVENT_USERS_QUERY].join(",");
        assert!(query(&max).is_ok());
    }

    #[test]
    fn body_parses_and_bounds_the_array() {
        let ok: EventUsers = serde_json::from_str(&format!(r#"["{ALICE}","{BOB}"]"#)).unwrap();
        assert_eq!(ok.0.len(), 2);
        assert!(serde_json::from_str::<EventUsers>("[]").is_err());
        assert!(serde_json::from_str::<EventUsers>(r#"["nope"]"#).is_err());
        let too_many = serde_json::to_string(&vec![ALICE; MAX_EVENT_USERS_BODY + 1]).unwrap();
        assert!(serde_json::from_str::<EventUsers>(&too_many).is_err());
    }
}
