use pubky_app_specs::PubkyId;
use std::collections::HashSet;

/// Prefixes that mark a user reference in post content, in the order the watcher scans
/// them. `pk:` is the legacy form, kept for backwards compatibility; `pubky` is the
/// current display form.
pub const MENTION_PREFIXES: [&str; 2] = ["pk:", "pubky"];

const USER_ID_LEN: usize = 52;

/// The users referenced in `content` with `prefix`, deduplicated, in order of first
/// appearance.
pub fn find_mentioned_ids(content: &str, prefix: &str) -> Vec<PubkyId> {
    let mut seen = HashSet::new();
    content
        .match_indices(prefix)
        .filter_map(|(start_idx, _)| {
            let user_id_start = start_idx + prefix.len();
            content
                .get(user_id_start..user_id_start + USER_ID_LEN)
                .and_then(|candidate| PubkyId::try_from(candidate).ok())
        })
        .filter(|id| seen.insert(id.to_string()))
        .collect()
}

/// Every user referenced in `content` under any of [`MENTION_PREFIXES`], deduplicated
/// across prefixes: the `mentioned_ids` stored on the post node.
pub fn mentioned_ids(content: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    MENTION_PREFIXES
        .iter()
        .flat_map(|prefix| find_mentioned_ids(content, prefix))
        .map(|id| id.to_string())
        .filter(|id| seen.insert(id.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALICE: &str = "ep441mndnsjeesenwz78r9paepm6e4kqm4ggiyy9uzpoe43eu9ny";
    const BOB: &str = "f5tcy5gtgzshipr6pag6cn9uski3s8tjare7wd3n7enmyokgjk1o";

    #[test]
    fn finds_ids_per_prefix() {
        let content = format!("hi pk:{ALICE} and pubky{BOB}, again pk:{ALICE}");
        let ids = |prefix| -> Vec<String> {
            find_mentioned_ids(&content, prefix)
                .into_iter()
                .map(|id| id.to_string())
                .collect()
        };
        assert_eq!(ids("pk:"), vec![ALICE]);
        assert_eq!(ids("pubky"), vec![BOB]);
    }

    #[test]
    fn mentioned_ids_dedups_across_prefixes() {
        let content = format!("pk:{ALICE} pubky{ALICE} pubky{BOB}");
        assert_eq!(mentioned_ids(&content), vec![ALICE, BOB]);
    }

    #[test]
    fn ignores_invalid_keys() {
        assert!(mentioned_ids("pk:notakey pubky").is_empty());
    }
}
