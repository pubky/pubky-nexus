use url::Url;

/// Placeholder written in place of a credential
pub const REDACTED: &str = "***";

/// Fragments of query parameter names that carry credentials, as in
/// `unix:///run/redis.sock?user=nexus&pass=secret` or `?access_token=secret`
const CREDENTIAL_QUERY_KEY_PARTS: [&str; 12] = [
    "auth", "bearer", "cred", "jwt", "key", "pass", "pw", "secret", "session", "sig", "token",
    "user",
];

/// Whether a query parameter name contains a credential fragment, ignoring case
fn is_credential_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    CREDENTIAL_QUERY_KEY_PARTS
        .iter()
        .any(|part| key.contains(part))
}

/// Returns `url` with its credentials replaced by [`REDACTED`], for logs and error messages.
///
/// The whole userinfo (`user:password@`) is replaced, since some providers put the secret in
/// the username. Credential query parameters are replaced too, and so is the whole fragment,
/// which Redis and Neo4j don't use but which can't be told apart from a secret. A string that
/// doesn't parse as a URL with an authority or a path, like `user:secret@host`, is replaced as
/// a whole, for the same reason.
pub fn redact_url(url: &str) -> String {
    let parsed = Url::parse(url).ok().filter(|u| !u.cannot_be_a_base());
    let Some(mut parsed) = parsed else {
        return REDACTED.to_string();
    };

    if !parsed.username().is_empty() || parsed.password().is_some() {
        // Only fails for URLs that can't hold credentials, so there is nothing to redact then
        let _ = parsed.set_password(None);
        let _ = parsed.set_username(REDACTED);
    }

    let has_credential_query = parsed.query_pairs().any(|(key, _)| is_credential_key(&key));
    if has_credential_query {
        let pairs: Vec<(String, String)> = parsed
            .query_pairs()
            .map(|(key, value)| {
                let value = match is_credential_key(&key) {
                    true => REDACTED.to_string(),
                    false => value.into_owned(),
                };
                (key.into_owned(), value)
            })
            .collect();
        parsed.query_pairs_mut().clear().extend_pairs(pairs);
    }

    if parsed
        .fragment()
        .is_some_and(|fragment| !fragment.is_empty())
    {
        parsed.set_fragment(Some(REDACTED));
    }

    parsed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_without_credentials_is_unchanged() {
        assert_eq!(
            redact_url("redis://localhost:6379"),
            "redis://localhost:6379"
        );
        assert_eq!(redact_url("bolt://localhost:7687"), "bolt://localhost:7687");
    }

    #[test]
    fn userinfo_is_redacted() {
        assert_eq!(
            redact_url("redis://default:s3cret@cache.example.com:6379/0"),
            "redis://***@cache.example.com:6379/0"
        );
        assert_eq!(
            redact_url("neo4j+s://neo4j:s3cret@graph.example.com:7687"),
            "neo4j+s://***@graph.example.com:7687"
        );
    }

    #[test]
    fn password_only_userinfo_is_redacted() {
        assert_eq!(
            redact_url("rediss://:s3cret@cache.example.com:6380"),
            "rediss://***@cache.example.com:6380"
        );
    }

    #[test]
    fn username_only_userinfo_is_redacted() {
        assert_eq!(
            redact_url("redis://s3cret@cache.example.com"),
            "redis://***@cache.example.com"
        );
    }

    #[test]
    fn percent_encoded_password_is_redacted() {
        let redacted = redact_url("redis://user:p%40ss%3Aword@cache.example.com:6379");
        assert_eq!(redacted, "redis://***@cache.example.com:6379");
    }

    #[test]
    fn credential_query_parameters_are_redacted() {
        assert_eq!(
            redact_url("unix:///run/redis.sock?db=1&user=nexus&pass=s3cret"),
            "unix:///run/redis.sock?db=1&user=***&pass=***"
        );
        assert_eq!(
            redact_url("redis://localhost:6379/?protocol=resp3&password=s3cret"),
            "redis://localhost:6379/?protocol=resp3&password=***"
        );
    }

    #[test]
    fn token_and_key_query_parameters_are_redacted() {
        assert_eq!(
            redact_url("redis://localhost:6379/?token=s3cret&db=2"),
            "redis://localhost:6379/?token=***&db=2"
        );
        assert_eq!(
            redact_url("neo4j://graph.example.com:7687?access_token=s3cret&api_key=s3cret"),
            "neo4j://graph.example.com:7687?access_token=***&api_key=***"
        );
        assert_eq!(
            redact_url("redis://localhost/?client_secret=s3cret&auth=s3cret&sig=s3cret"),
            "redis://localhost/?client_secret=***&auth=***&sig=***"
        );
    }

    #[test]
    fn credential_query_keys_match_ignoring_case() {
        assert_eq!(
            redact_url("redis://localhost/?TOKEN=s3cret&Password=s3cret&ApiKey=s3cret"),
            "redis://localhost/?TOKEN=***&Password=***&ApiKey=***"
        );
    }

    #[test]
    fn percent_encoded_credential_query_key_is_redacted() {
        assert_eq!(
            redact_url("redis://localhost/?t%6Fken=s3cret"),
            "redis://localhost/?token=***"
        );
    }

    #[test]
    fn userinfo_and_query_credentials_are_redacted_together() {
        assert_eq!(
            redact_url("rediss://default:s3cret@cache.example.com:6380/0?token=s3cret"),
            "rediss://***@cache.example.com:6380/0?token=***"
        );
    }

    #[test]
    fn jwt_bearer_and_pw_query_parameters_are_redacted() {
        assert_eq!(
            redact_url("neo4j://graph.example.com:7687?jwt=s3cret&bearer=s3cret&pw=s3cret"),
            "neo4j://graph.example.com:7687?jwt=***&bearer=***&pw=***"
        );
    }

    #[test]
    fn fragment_is_redacted() {
        assert_eq!(
            redact_url("redis://localhost:6379/#token=s3cret"),
            "redis://localhost:6379/#***"
        );
        assert_eq!(
            redact_url("bolt://graph.example.com:7687?db=neo4j#s3cret"),
            "bolt://graph.example.com:7687?db=neo4j#***"
        );
    }

    #[test]
    fn empty_fragment_is_unchanged() {
        assert_eq!(
            redact_url("redis://localhost:6379/#"),
            "redis://localhost:6379/#"
        );
    }

    #[test]
    fn unparseable_url_is_redacted_whole() {
        assert_eq!(redact_url("user:s3cret@localhost"), REDACTED);
        assert_eq!(redact_url(""), REDACTED);
    }
}
