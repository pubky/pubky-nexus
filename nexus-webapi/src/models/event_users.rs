use std::collections::HashSet;

use pubky_app_specs::PubkyId;
use serde::Deserialize;
use utoipa::ToSchema;

use super::bounded_vec;

/// Most authors `POST /v0/events/by_users` accepts.
const MAX_EVENT_USERS_BODY: usize = 1_000;

/// JSON array of Pubky ids for `POST /v0/events/by_users` (1–1,000).
#[derive(Debug, ToSchema)]
#[schema(value_type = Vec<String>)]
pub struct EventUsers(pub HashSet<String>);

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

    fn parse(json: &str) -> Result<EventUsers, serde_json::Error> {
        serde_json::from_str(json)
    }

    #[test]
    fn parses_ids_into_a_set() {
        let users = parse(&format!(r#"["{ALICE}","{BOB}","{ALICE}"]"#)).unwrap();
        assert_eq!(users.0, HashSet::from([ALICE.to_string(), BOB.to_string()]));
    }

    #[test]
    fn rejects_empty_invalid_and_too_many() {
        assert!(parse("[]").is_err());
        assert!(parse(r#"["nope"]"#).is_err());
        let max = serde_json::to_string(&vec![ALICE; MAX_EVENT_USERS_BODY]).unwrap();
        assert!(parse(&max).is_ok());
        let too_many = serde_json::to_string(&vec![ALICE; MAX_EVENT_USERS_BODY + 1]).unwrap();
        assert!(parse(&too_many).is_err());
    }
}
