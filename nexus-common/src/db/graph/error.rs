use thiserror::Error;

use crate::types::DynError;

pub type GraphResult<T> = std::result::Result<T, GraphError>;

#[derive(Error, Debug)]
pub enum GraphError {
    #[error("Connection not initialized")]
    ConnectionNotInitialized,

    #[error("Query failed: {0}")]
    QueryFailed(#[from] neo4rs::Error),

    #[error("Query timeout")]
    QueryTimeout,

    #[error("Query build error: {0}")]
    QueryBuildError(String),

    #[error("Serialization failed: {0}")]
    SerializationFailed(#[source] DynError),

    #[error("Deserialization failed: {0}")]
    DeserializationFailed(#[source] DynError),

    #[error("URI parse error: {0}")]
    UriParseError(String),

    #[error("Invalid resource type: {0}")]
    InvalidResourceType(String),

    /// A keyset scan could not advance, so it would skip or refetch rows forever.
    #[error("{context}: keyset scan cannot advance ({detail})")]
    KeysetScanStalled { context: String, detail: String },

    #[error("Generic: {0}")]
    Generic(String),
}

/// Neo4j rejected a write that breaks a schema constraint. Retrying the batch can
/// never fix it, so it is a problem with that one event, not with the database.
const CONSTRAINT_VIOLATION_CODE: &str = "Neo.ClientError.Schema.ConstraintValidationFailed";

impl GraphError {
    pub fn should_not_retry_now(&self) -> bool {
        match self {
            GraphError::ConnectionNotInitialized => true,
            GraphError::QueryFailed(e) => !is_constraint_violation(e),
            _ => false,
        }
    }
}

fn is_constraint_violation(error: &neo4rs::Error) -> bool {
    match error {
        // `Graph::run` reports the server FAILURE as a typed error.
        neo4rs::Error::Neo4j(e) => e.code() == CONSTRAINT_VIOLATION_CODE,
        // `Graph::execute` sends only RUN; the write fails on PULL, and neo4rs 0.8 reports
        // that FAILURE as `UnexpectedMessage` holding its Debug output, code included.
        neo4rs::Error::UnexpectedMessage(msg) => msg.contains(CONSTRAINT_VIOLATION_CODE),
        _ => false,
    }
}

impl From<neo4rs::DeError> for GraphError {
    fn from(e: neo4rs::DeError) -> Self {
        GraphError::DeserializationFailed(Box::new(e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pull_failure(code: &str) -> GraphError {
        // The shape neo4rs 0.8 gives a FAILURE answering PULL (`RowStream::next`).
        GraphError::QueryFailed(neo4rs::Error::UnexpectedMessage(format!(
            "unexpected response for PULL: Ok(Failure(Failure {{ metadata: BoltMap {{ value: {{BoltString {{ value: \"code\" }}: String(BoltString {{ value: \"{code}\" }})}} }} }}))"
        )))
    }

    #[test]
    fn constraint_violation_on_pull_does_not_stop_the_batch() {
        assert!(
            !pull_failure("Neo.ClientError.Schema.ConstraintValidationFailed")
                .should_not_retry_now()
        );
        assert!(
            pull_failure("Neo.TransientError.Transaction.DeadlockDetected").should_not_retry_now()
        );
        assert!(pull_failure("Neo.ClientError.Statement.SyntaxError").should_not_retry_now());
    }

    #[test]
    fn connection_level_query_failures_still_stop_the_batch() {
        assert!(GraphError::QueryFailed(neo4rs::Error::ConnectionError).should_not_retry_now());
        assert!(GraphError::ConnectionNotInitialized.should_not_retry_now());
        assert!(!GraphError::QueryTimeout.should_not_retry_now());
    }
}

/// Real constraint violations, through both ways Nexus sends a write.
#[cfg(test)]
mod db_tests {
    use crate::db::{exec_single_row, fetch_row_from_graph, graph::Query};
    use crate::types::DynError;
    use crate::{StackConfig, StackManager};

    /// Two nodes under the global `uniqueTakedownUri` constraint.
    fn duplicate_takedown(uri: &str) -> Query {
        Query::new(
            "test_duplicate_takedown",
            "CREATE (:Takedown {uri: $uri}) CREATE (:Takedown {uri: $uri}) RETURN true AS flag",
        )
        .param("uri", uri)
    }

    #[tokio_shared_rt::test(shared)]
    async fn constraint_violation_is_per_event_on_execute_and_run() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let uri = format!(
            "pubky://{}/pub/pubky.app/posts/0034FJVRPG4V0",
            pubky::Keypair::random().public_key().to_z32()
        );

        let on_execute = fetch_row_from_graph(duplicate_takedown(&uri))
            .await
            .expect_err("the constraint rejects the write");
        assert!(!on_execute.should_not_retry_now(), "{on_execute}");

        let on_run = exec_single_row(duplicate_takedown(&uri))
            .await
            .expect_err("the constraint rejects the write");
        assert!(!on_run.should_not_retry_now(), "{on_run}");

        Ok(())
    }

    /// Posts may share an id across authors, but never a `uri` (one author, one id).
    #[tokio_shared_rt::test(shared)]
    async fn post_uri_is_unique() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let post_id = "0034FJVRPG4V0";
        let uri = |author: &str| format!("pubky://{author}/pub/pubky.app/posts/{post_id}");
        let a = pubky::Keypair::random().public_key().to_z32();
        let b = pubky::Keypair::random().public_key().to_z32();
        let two_posts = |uri_a: String, uri_b: String| {
            Query::new(
                "test_two_posts",
                "CREATE (:Post {id: $id, uri: $uri_a}) CREATE (:Post {id: $id, uri: $uri_b})",
            )
            .param("id", post_id)
            .param("uri_a", uri_a)
            .param("uri_b", uri_b)
        };
        let cleanup = Query::new(
            "test_two_posts_cleanup",
            "MATCH (p:Post) WHERE p.uri IN $uris DETACH DELETE p",
        )
        .param("uris", vec![uri(&a), uri(&b)]);

        exec_single_row(two_posts(uri(&a), uri(&b))).await?;
        exec_single_row(cleanup).await?;

        let err = exec_single_row(two_posts(uri(&a), uri(&a)))
            .await
            .expect_err("uniquePostUri rejects the second post");
        assert!(!err.should_not_retry_now(), "{err}");

        Ok(())
    }
}
