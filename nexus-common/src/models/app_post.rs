//! Generic namespace post paths and envelope validation. No graph projection.

use pubky_social_specs::traits::{TimestampId, Validatable};
use pubky_social_specs::{
    ParsedUri, PostEnvelope, PostKind, PubkyId, Resource, Root, ValidationCtx, Visibility,
};
use serde::{Deserialize, Serialize};

pub fn valid_kind(value: &str) -> bool {
    (1..=32).contains(&value.len())
        && value.as_bytes()[0].is_ascii_lowercase()
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

/// Sub-spec namespace rule, pending the validator in the pinned specs crate.
/// Social has its own reader and is never an app namespace.
pub fn valid_namespace(value: &str) -> bool {
    value != "social"
        && (1..=32).contains(&value.len())
        && value.as_bytes()[0].is_ascii_lowercase()
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// The app owns the vocabulary and content semantics. Nexus accepts bounded lowercase
/// kind tokens, leaving content and extensions entirely to the app.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct AppKind(pub String);

impl PostKind for AppKind {
    fn is_known(&self) -> bool {
        valid_kind(&self.0)
    }
    fn validate_content(
        &self,
        _post: &PostEnvelope<Self>,
        _ctx: &ValidationCtx,
    ) -> Result<(), String> {
        Ok(())
    }
}

pub type AppPostInput = PostEnvelope<AppKind>;

/// One logical post and, for stored events, its version leaf. The social reader
/// regards app paths as foreign; its post path parser still owns the shared layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPostUri {
    pub author: PubkyId,
    pub namespace: String,
    pub id: String,
    pub version: Option<String>,
    pub label: Option<String>,
}

impl AppPostUri {
    pub fn parse(uri: &str) -> Option<Self> {
        let parsed = ParsedUri::try_from(uri).ok()?;
        if parsed.visibility != Visibility::Public || parsed.try_to_uri_str().ok()?.as_str() != uri
        {
            return None;
        }
        let Resource::Foreign {
            namespace,
            version: Some(epoch),
            rest,
        } = parsed.resource
        else {
            return None;
        };
        if !valid_namespace(&namespace) || epoch != "v1" {
            return None;
        }
        // Reuse the pinned reader's timestamp, leaf, slug and segment rules rather
        // than maintaining a second version-path grammar for app posts.
        let mirror = format!(
            "pubky://{}/pub/social/v1/{}",
            parsed.user_id,
            rest.join("/")
        );
        let social = ParsedUri::try_from(mirror.as_str()).ok()?;
        if social.try_to_uri_str().ok()?.as_str() != mirror {
            return None;
        }
        let Resource::Post { id, version, label } = social.resource else {
            return None;
        };
        Some(Self {
            author: parsed.user_id,
            namespace,
            id,
            version,
            label,
        })
    }

    pub fn path(&self) -> String {
        format!("/pub/{}/v1/posts/{}", self.namespace, self.id)
    }
    pub fn uri(&self) -> String {
        format!("pubky://{}{}", self.author, self.path())
    }
    pub fn version_uri(&self) -> String {
        format!("pubky://{}{}", self.author, self.version_path())
    }
    pub fn version_path(&self) -> String {
        match &self.version {
            Some(version) => AppPostInput::version_path(
                &self.namespace,
                Root::Pub,
                &self.id,
                version,
                self.label.as_deref(),
            ),
            None => self.path(),
        }
    }

    pub fn read(&self, bytes: &[u8]) -> Result<AppPostInput, String> {
        let version = self
            .version
            .as_deref()
            .ok_or("Validation Error: a versionless post reference is never a stored object")?;
        let ctx = ValidationCtx { root: Root::Pub };
        let post = <AppPostInput as Validatable>::try_from(bytes, &self.id, &ctx)?;
        // Same version rules as PubkySocialObject::from_resource at the pinned revision.
        post.validate_id(version)?;
        if version.as_bytes() < self.id.as_bytes() {
            return Err("Validation Error: version is older than the post id".into());
        }
        post.check_references(&ctx, Some(&self.author))?;
        Ok(post)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const AUTHOR: &str = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
    const ID: &str = "0032SSN7Q4EVG";
    const EDIT: &str = "0032SSN7Q4EVJ";

    fn stored() -> AppPostUri {
        AppPostUri::parse(&format!(
            "pubky://{AUTHOR}/pub/example/v1/posts/{ID}/{EDIT}.json"
        ))
        .unwrap()
    }

    #[test]
    fn canonical_versions_share_a_logical_identity() {
        let plain = stored();
        let labelled =
            AppPostUri::parse(&plain.version_uri().replace(".json", "-my-article.json")).unwrap();
        assert_eq!(plain.uri(), labelled.uri());
        assert_ne!(plain.version_uri(), labelled.version_uri());
        assert_eq!(AppPostUri::parse(&plain.uri()).unwrap().version, None);
        for uri in [
            plain.version_uri().replace("pubky://", "pubky"),
            plain.version_uri().replace("/pub/", "/priv/"),
            plain.version_uri().replace(".json", ".txt"),
            plain.version_uri().replace("/example/", "/pubky.app/"),
            plain.version_uri().replace("/example/", "/social/"),
            format!("{}/extra", plain.version_uri()),
        ] {
            assert!(AppPostUri::parse(&uri).is_none(), "{uri}");
        }
    }

    #[test]
    fn app_namespaces_follow_the_subspec_rule_without_an_allowlist() {
        for namespace in ["x", "example-2", "abcdefghijklmnopqrstuvwxyz012345"] {
            assert!(valid_namespace(namespace), "{namespace}");
            let uri = format!("pubky://{AUTHOR}/pub/{namespace}/v1/posts/{ID}/{EDIT}.json");
            assert_eq!(AppPostUri::parse(&uri).unwrap().namespace, namespace);
        }
        for namespace in [
            "",
            "social",
            "pubky.app",
            "example.app",
            "example_app",
            "Example",
            "1example",
            "-example",
            "exámple",
            "abcdefghijklmnopqrstuvwxyz0123456",
        ] {
            assert!(!valid_namespace(namespace), "{namespace}");
            let uri = format!("pubky://{AUTHOR}/pub/{namespace}/v1/posts/{ID}/{EDIT}.json");
            assert!(AppPostUri::parse(&uri).is_none(), "{namespace}");
        }
    }

    #[test]
    fn shared_envelope_validates_fields_but_leaves_app_content_and_extensions_opaque() {
        let path = stored();
        let body = json!({"kind":"my_kind-2", "content":"  arbitrary, not JSON  ", "ext":{"future": [null, 7]}});
        let post = path.read(&serde_json::to_vec(&body).unwrap()).unwrap();
        assert_eq!(post.content, "  arbitrary, not JSON  ");
        assert_eq!(post.extra["ext"], body["ext"]);
        // V1 ids belong to the path; no body id or private copy of the envelope is required.
        for (field, value) in [
            ("kind", json!("Upper")),
            ("kind", json!("x".repeat(33))),
            ("content", json!({})),
            ("attachments", json!(null)),
            ("parent", json!("relative/path")),
            (
                "attachments",
                json!([{"uri":"https://example.com/x", "name":" "}]),
            ),
            ("lock", json!("https://example.com/lock")),
            ("ext", json!("x".repeat(512 * 1024))),
        ] {
            let mut invalid = body.clone();
            invalid[field] = value;
            assert!(
                path.read(&serde_json::to_vec(&invalid).unwrap()).is_err(),
                "{field}"
            );
        }
        let mut old_version = path.clone();
        old_version.id = EDIT.into();
        old_version.version = Some(ID.into());
        assert!(old_version
            .read(&serde_json::to_vec(&body).unwrap())
            .is_err());
        assert!(AppPostUri::parse(&path.uri())
            .unwrap()
            .read(&serde_json::to_vec(&body).unwrap())
            .is_err());
    }
}
