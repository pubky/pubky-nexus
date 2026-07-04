//! Neighborhood graph view for the interactive graph explorer.
//!
//! Serves `GET /v0/graph/{kind}/{id}`: a typed node-link graph around a center
//! entity (user, post, or tag label). Node ids are kind-prefixed so they stay
//! globally unique in one namespace: `user:{pubky}`, `post:{author}:{post_id}`,
//! `tag:{label}`. Tag nodes are synthetic hubs: there is no `Tag` node label in
//! Neo4j, only `TAGGED` relationships carrying a `label` property.
//!
//! These are multi-hop aggregate reads, so they hit Neo4j directly (like the
//! influencers/recommended queries) instead of the per-entity Redis indexes.

use std::collections::HashSet;

use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::db::{fetch_row_from_graph, queries};
use crate::models::error::ModelResult;

/// Default cap on the primary neighbor class of a graph query.
pub const GRAPH_DEFAULT_LIMIT: usize = 30;
/// Hard cap on the primary neighbor class of a graph query.
pub const GRAPH_MAX_LIMIT: usize = 50;
/// Global cap on hop-2 users of a depth-2 user graph (applied after DISTINCT,
/// not per hop-1 node, so depth-2 payloads stay bounded by construction).
pub const GRAPH_HOP2_TOTAL_CAP: usize = 120;
/// Cap on the center's most recent posts included in a user graph.
pub const GRAPH_POSTS_LIMIT: usize = 10;
/// Cap on the hottest tag labels included in a user or post graph.
pub const GRAPH_TAGS_LIMIT: usize = 10;

/// Which node kinds a graph response should include. The center is always kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphKinds {
    pub user: bool,
    pub post: bool,
    pub tag: bool,
}

impl Default for GraphKinds {
    fn default() -> Self {
        Self {
            user: true,
            post: true,
            tag: true,
        }
    }
}

// Deserializes from a CSV of kind names, e.g. "user,post,tag". Unknown names
// are rejected so a malformed query string surfaces as a 400 instead of being
// silently ignored; duplicates are tolerated.
impl<'de> Deserialize<'de> for GraphKinds {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let mut kinds = GraphKinds {
            user: false,
            post: false,
            tag: false,
        };
        for part in s.split(',') {
            match part.trim() {
                "user" => kinds.user = true,
                "post" => kinds.post = true,
                "tag" => kinds.tag = true,
                other => return Err(de::Error::custom(format!("unknown node kind: '{other}'"))),
            }
        }
        Ok(kinds)
    }
}

/// FOLLOWS traversal depth of a user-centered graph, validated to `1..=2` at
/// construction (mirrors `WotDepth`). Depth 2 pulls in the globally-capped
/// hop-2 neighborhood, so the bound is enforced for every caller rather than
/// at the web layer alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphDepth(u8);

impl Default for GraphDepth {
    /// Hop-2 payloads are much heavier, so default to the immediate neighborhood.
    fn default() -> Self {
        GraphDepth(1)
    }
}

impl GraphDepth {
    pub const MIN: u8 = 1;
    pub const MAX: u8 = 2;

    /// Validates that `depth` is within `1..=2`.
    pub fn new(depth: u8) -> Result<Self, String> {
        if (Self::MIN..=Self::MAX).contains(&depth) {
            Ok(GraphDepth(depth))
        } else {
            Err(format!(
                "'depth' must be between {} and {}",
                Self::MIN,
                Self::MAX
            ))
        }
    }

    /// The underlying depth value.
    pub fn get(self) -> u8 {
        self.0
    }
}

// Deserialize through `new` so the `1..=2` invariant holds for every input.
// Query params arrive as strings, so parse via String (as `BoundedLimit` does).
impl<'de> Deserialize<'de> for GraphDepth {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let depth: u8 = s.parse().map_err(de::Error::custom)?;
        GraphDepth::new(depth).map_err(de::Error::custom)
    }
}

#[derive(Serialize, Deserialize, ToSchema, Debug, Clone)]
pub struct UserGraphNode {
    /// Prefixed graph id: `user:{pubky}`
    pub id: String,
    pub pubky: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
}

#[derive(Serialize, Deserialize, ToSchema, Debug, Clone)]
pub struct PostGraphNode {
    /// Prefixed graph id: `post:{author}:{post_id}`
    pub id: String,
    pub author_id: String,
    pub post_id: String,
    /// First 100 characters of the post content
    pub content: String,
    pub post_kind: String,
    pub indexed_at: i64,
}

#[derive(Serialize, Deserialize, ToSchema, Debug, Clone)]
pub struct TagGraphNode {
    /// Prefixed graph id: `tag:{label}`
    pub id: String,
    pub label: String,
    /// Times the label is used within the returned neighborhood (or globally
    /// for a tag-centered graph)
    pub count: u64,
}

#[derive(Serialize, Deserialize, ToSchema, Debug, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GraphNode {
    User(UserGraphNode),
    Post(PostGraphNode),
    Tag(TagGraphNode),
}

impl GraphNode {
    pub fn id(&self) -> &str {
        match self {
            GraphNode::User(n) => &n.id,
            GraphNode::Post(n) => &n.id,
            GraphNode::Tag(n) => &n.id,
        }
    }

    fn enabled(&self, kinds: GraphKinds) -> bool {
        match self {
            GraphNode::User(_) => kinds.user,
            GraphNode::Post(_) => kinds.post,
            GraphNode::Tag(_) => kinds.tag,
        }
    }
}

#[derive(Serialize, Deserialize, ToSchema, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GraphEdgeType {
    Follows,
    Authored,
    Tagged,
    Replied,
    Reposted,
    Mentioned,
}

impl GraphEdgeType {
    /// An edge type is only meaningful when the node kinds it relates are
    /// requested, e.g. `kinds=user` leaves no room for TAGGED or AUTHORED.
    fn enabled(&self, kinds: GraphKinds) -> bool {
        match self {
            GraphEdgeType::Follows => kinds.user,
            GraphEdgeType::Tagged => kinds.tag,
            GraphEdgeType::Authored
            | GraphEdgeType::Replied
            | GraphEdgeType::Reposted
            | GraphEdgeType::Mentioned => kinds.post,
        }
    }
}

#[derive(Serialize, Deserialize, ToSchema, Debug, Clone)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    #[serde(rename = "type")]
    pub edge_type: GraphEdgeType,
    /// Present only on TAGGED edges
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// When the relationship was indexed; drives the client's time filters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indexed_at: Option<i64>,
}

#[derive(Serialize, Deserialize, ToSchema, Debug, Default)]
pub struct GraphView {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

fn user_gid(pubky: &str) -> String {
    format!("user:{pubky}")
}

fn post_gid(author_id: &str, post_id: &str) -> String {
    format!("post:{author_id}:{post_id}")
}

fn tag_gid(label: &str) -> String {
    format!("tag:{label}")
}

// ---------------------------------------------------------------------------
// Row-shape structs matching the Cypher RETURN maps in queries::get
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct UserRow {
    id: String,
    name: Option<String>,
    image: Option<String>,
}

#[derive(Deserialize)]
struct PostRow {
    id: String,
    content: String,
    kind: String,
    indexed_at: i64,
}

/// A post carrying its author inline (tag- and post-centered graphs, where the
/// author is not the center).
#[derive(Deserialize)]
struct AuthoredPostRow {
    id: String,
    author_id: String,
    author_name: Option<String>,
    author_image: Option<String>,
    content: String,
    kind: String,
    indexed_at: i64,
}

/// A parent post of the center, with the relationship that points at it.
#[derive(Deserialize)]
struct ParentPostRow {
    rel: String,
    id: String,
    author_id: String,
    author_name: Option<String>,
    author_image: Option<String>,
    content: String,
    kind: String,
    indexed_at: i64,
}

#[derive(Deserialize)]
struct TagRow {
    label: String,
    count: u64,
    /// Pre-prefixed graph ids of the entities the label is on
    targets: Vec<String>,
}

#[derive(Deserialize)]
struct LabelRow {
    label: String,
    count: u64,
}

/// Accumulates nodes/edges with id-level dedup, since the same user can enter
/// the graph through several roles (neighbor, tagger, reply author, ...).
#[derive(Default)]
struct GraphBuilder {
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
    seen: HashSet<String>,
}

impl GraphBuilder {
    fn push_user(&mut self, pubky: &str, name: Option<String>, image: Option<String>) -> String {
        let id = user_gid(pubky);
        if self.seen.insert(id.clone()) {
            self.nodes.push(GraphNode::User(UserGraphNode {
                id: id.clone(),
                pubky: pubky.to_string(),
                name: name.unwrap_or_default(),
                image,
            }));
        }
        id
    }

    fn push_post(&mut self, author_id: &str, post: PostRow) -> String {
        let id = post_gid(author_id, &post.id);
        if self.seen.insert(id.clone()) {
            self.nodes.push(GraphNode::Post(PostGraphNode {
                id: id.clone(),
                author_id: author_id.to_string(),
                post_id: post.id,
                content: post.content,
                post_kind: post.kind,
                indexed_at: post.indexed_at,
            }));
        }
        id
    }

    /// Pushes an authored post plus its author node and the AUTHORED edge.
    fn push_authored_post(&mut self, post: AuthoredPostRow) -> String {
        let author_id = post.author_id;
        let author_gid = self.push_user(&author_id, post.author_name, post.author_image);
        let post_gid = self.push_post(
            &author_id,
            PostRow {
                id: post.id,
                content: post.content,
                kind: post.kind,
                indexed_at: post.indexed_at,
            },
        );
        self.edge(&author_gid, &post_gid, GraphEdgeType::Authored, None);
        post_gid
    }

    fn push_tag(&mut self, label: &str, count: u64) -> String {
        let id = tag_gid(label);
        if self.seen.insert(id.clone()) {
            self.nodes.push(GraphNode::Tag(TagGraphNode {
                id: id.clone(),
                label: label.to_string(),
                count,
            }));
        }
        id
    }

    fn edge(&mut self, source: &str, target: &str, edge_type: GraphEdgeType, label: Option<&str>) {
        self.edge_at(source, target, edge_type, label, None);
    }

    fn edge_at(
        &mut self,
        source: &str,
        target: &str,
        edge_type: GraphEdgeType,
        label: Option<&str>,
        indexed_at: Option<i64>,
    ) {
        self.edges.push(GraphEdge {
            source: source.to_string(),
            target: target.to_string(),
            edge_type,
            label: label.map(str::to_string),
            indexed_at,
        });
    }

    /// Drops nodes of unrequested kinds (center exempt) and edges that lost an
    /// endpoint or whose type is meaningless for the requested kinds. On
    /// non-user centers (`exempt_center_edges`), edges incident to the center
    /// survive type gating so kinds filtering cannot orphan the center from
    /// its own neighborhood. Edges are deduplicated: the same relationship can
    /// reach the builder through several roles (e.g. an author discovered as
    /// both reply author and repost author).
    fn build(self, kinds: GraphKinds, center_id: &str, exempt_center_edges: bool) -> GraphView {
        let nodes: Vec<GraphNode> = self
            .nodes
            .into_iter()
            .filter(|n| n.id() == center_id || n.enabled(kinds))
            .collect();
        let kept: HashSet<&str> = nodes.iter().map(|n| n.id()).collect();
        let mut seen_edges: HashSet<(String, String, GraphEdgeType, Option<String>)> =
            HashSet::new();
        let edges = self
            .edges
            .into_iter()
            .filter(|e| {
                let type_ok = e.edge_type.enabled(kinds)
                    || (exempt_center_edges && (e.source == center_id || e.target == center_id));
                type_ok
                    && kept.contains(e.source.as_str())
                    && kept.contains(e.target.as_str())
                    && seen_edges.insert((
                        e.source.clone(),
                        e.target.clone(),
                        e.edge_type,
                        e.label.clone(),
                    ))
            })
            .collect();
        GraphView { nodes, edges }
    }
}

impl GraphView {
    /// Neighborhood around a user: their undirected FOLLOWS neighbors (friends
    /// first, then most-followed), optional hop-2 users, their recent posts and
    /// hottest labels, plus every FOLLOWS/TAGGED/MENTIONED/REPLIED/REPOSTED
    /// edge among the returned entities. Returns `Ok(None)` for unknown users.
    pub async fn get_by_user(
        user_id: &str,
        depth: GraphDepth,
        limit: usize,
        kinds: GraphKinds,
    ) -> ModelResult<Option<Self>> {
        let hop2_limit = if depth.get() >= 2 {
            GRAPH_HOP2_TOTAL_CAP
        } else {
            0
        };
        // Excluded kinds are not fetched at all (LIMIT 0), not just filtered out.
        let posts_limit = if kinds.post { GRAPH_POSTS_LIMIT } else { 0 };
        let tags_limit = if kinds.tag { GRAPH_TAGS_LIMIT } else { 0 };

        let query = queries::get::graph_neighborhood_by_user(
            user_id,
            limit,
            hop2_limit,
            posts_limit,
            tags_limit,
        );
        let Some(row) = fetch_row_from_graph(query).await? else {
            return Ok(None);
        };

        let user_nodes = row.get::<Vec<UserRow>>("user_nodes")?;
        let post_nodes = row.get::<Vec<PostRow>>("post_nodes")?;
        let tag_nodes = row.get::<Vec<TagRow>>("tag_nodes")?;
        let follow_edges = row.get::<Vec<(String, String, Option<i64>)>>("follow_edges")?;
        let user_tag_edges =
            row.get::<Vec<(String, String, String, Option<i64>)>>("user_tag_edges")?;
        let mention_edges = row.get::<Vec<Vec<String>>>("mention_edges")?;
        let post_post_edges = row.get::<Vec<Vec<String>>>("post_post_edges")?;

        let mut builder = GraphBuilder::default();
        let center_id = user_gid(user_id);

        for user in user_nodes {
            builder.push_user(&user.id, user.name, user.image);
        }
        for post in post_nodes {
            let post_gid = builder.push_post(user_id, post);
            builder.edge(&center_id, &post_gid, GraphEdgeType::Authored, None);
        }
        for tag in tag_nodes {
            let tag_id = builder.push_tag(&tag.label, tag.count);
            for target in tag.targets {
                builder.edge(&tag_id, &target, GraphEdgeType::Tagged, Some(&tag.label));
            }
        }
        for (a, b, ts) in follow_edges {
            builder.edge_at(
                &user_gid(&a),
                &user_gid(&b),
                GraphEdgeType::Follows,
                None,
                ts,
            );
        }
        for (a, b, label, ts) in user_tag_edges {
            builder.edge_at(
                &user_gid(&a),
                &user_gid(&b),
                GraphEdgeType::Tagged,
                Some(&label),
                ts,
            );
        }
        for pair in mention_edges {
            if let [post_id, mentioned] = pair.as_slice() {
                builder.edge(
                    &post_gid(user_id, post_id),
                    &user_gid(mentioned),
                    GraphEdgeType::Mentioned,
                    None,
                );
            }
        }
        for triple in post_post_edges {
            if let [rel, from, to] = triple.as_slice() {
                let edge_type = match rel.as_str() {
                    "REPOSTED" => GraphEdgeType::Reposted,
                    _ => GraphEdgeType::Replied,
                };
                builder.edge(
                    &post_gid(user_id, from),
                    &post_gid(user_id, to),
                    edge_type,
                    None,
                );
            }
        }

        Ok(Some(builder.build(kinds, &center_id, false)))
    }

    /// Shortest undirected FOLLOWS path between two users (max 6 hops), nodes
    /// path-ordered from `from` to `to`. A user paired with themselves returns
    /// their single node. Returns `Ok(None)` when either user is unknown or no
    /// path exists within the cap.
    pub async fn get_path(from: &str, to: &str) -> ModelResult<Option<Self>> {
        let mut builder = GraphBuilder::default();

        if from == to {
            let query = queries::get::graph_user_card(from);
            let Some(row) = fetch_row_from_graph(query).await? else {
                return Ok(None);
            };
            let user = row.get::<UserRow>("user_node")?;
            builder.push_user(&user.id, user.name, user.image);
            return Ok(Some(GraphView {
                nodes: builder.nodes,
                edges: builder.edges,
            }));
        }

        let query = queries::get::graph_shortest_path(from, to);
        let Some(row) = fetch_row_from_graph(query).await? else {
            return Ok(None);
        };

        let path_nodes = row.get::<Vec<UserRow>>("path_nodes")?;
        let path_edges = row.get::<Vec<(String, String, Option<i64>)>>("path_edges")?;

        for user in path_nodes {
            builder.push_user(&user.id, user.name, user.image);
        }
        for (a, b, ts) in path_edges {
            builder.edge_at(
                &user_gid(&a),
                &user_gid(&b),
                GraphEdgeType::Follows,
                None,
                ts,
            );
        }

        // Path views are user-only by construction; no kind filtering needed
        Ok(Some(GraphView {
            nodes: builder.nodes,
            edges: builder.edges,
        }))
    }

    /// Whether a (non-deleted) user exists in the graph.
    pub async fn user_exists(user_id: &str) -> ModelResult<bool> {
        let query = queries::get::graph_user_card(user_id);
        Ok(fetch_row_from_graph(query).await?.is_some())
    }

    /// Neighborhood around a tag label: its most active taggers, tagged users
    /// and recent tagged posts (as hub edges out of the synthetic tag node),
    /// plus FOLLOWS edges among the returned taggers. Returns `Ok(None)` for
    /// labels with no TAGGED relationship.
    pub async fn get_by_tag(
        label: &str,
        limit: usize,
        kinds: GraphKinds,
    ) -> ModelResult<Option<Self>> {
        let posts_limit = if kinds.post { GRAPH_POSTS_LIMIT } else { 0 };

        let query = queries::get::graph_neighborhood_by_tag(label, limit, posts_limit);
        let Some(row) = fetch_row_from_graph(query).await? else {
            return Ok(None);
        };

        let total = row.get::<u64>("total")?;
        let tagger_nodes = row.get::<Vec<UserRow>>("tagger_nodes")?;
        let tagged_user_nodes = row.get::<Vec<UserRow>>("tagged_user_nodes")?;
        let post_nodes = row.get::<Vec<AuthoredPostRow>>("post_nodes")?;
        let follow_edges = row.get::<Vec<(String, String, Option<i64>)>>("follow_edges")?;

        let mut builder = GraphBuilder::default();
        let center_id = builder.push_tag(label, total);

        for tagger in tagger_nodes {
            let tagger_gid = builder.push_user(&tagger.id, tagger.name, tagger.image);
            builder.edge(&tagger_gid, &center_id, GraphEdgeType::Tagged, Some(label));
        }
        for tagged in tagged_user_nodes {
            let tagged_gid = builder.push_user(&tagged.id, tagged.name, tagged.image);
            builder.edge(&center_id, &tagged_gid, GraphEdgeType::Tagged, Some(label));
        }
        for post in post_nodes {
            let post_gid = builder.push_authored_post(post);
            builder.edge(&center_id, &post_gid, GraphEdgeType::Tagged, Some(label));
        }
        for (a, b, ts) in follow_edges {
            builder.edge_at(
                &user_gid(&a),
                &user_gid(&b),
                GraphEdgeType::Follows,
                None,
                ts,
            );
        }

        Ok(Some(builder.build(kinds, &center_id, true)))
    }

    /// Neighborhood around a post: its author, recent replies and reposts with
    /// their authors, parents, mentioned users, and labels on it. Returns
    /// `Ok(None)` for unknown posts.
    pub async fn get_by_post(
        author_id: &str,
        post_id: &str,
        limit: usize,
        kinds: GraphKinds,
    ) -> ModelResult<Option<Self>> {
        let tags_limit = if kinds.tag { GRAPH_TAGS_LIMIT } else { 0 };

        let query = queries::get::graph_neighborhood_by_post(author_id, post_id, limit, tags_limit);
        let Some(row) = fetch_row_from_graph(query).await? else {
            return Ok(None);
        };

        let author = row.get::<UserRow>("author_node")?;
        let center = row.get::<PostRow>("center_node")?;
        let replies = row.get::<Vec<AuthoredPostRow>>("replies")?;
        let reposts = row.get::<Vec<AuthoredPostRow>>("reposts")?;
        let parents = row.get::<Vec<ParentPostRow>>("parents")?;
        let mentioned_nodes = row.get::<Vec<UserRow>>("mentioned_nodes")?;
        let labels = row.get::<Vec<LabelRow>>("labels")?;

        let mut builder = GraphBuilder::default();
        let author_gid = builder.push_user(&author.id, author.name, author.image);
        let center_id = builder.push_post(author_id, center);
        builder.edge(&author_gid, &center_id, GraphEdgeType::Authored, None);

        for reply in replies {
            let reply_gid = builder.push_authored_post(reply);
            builder.edge(&reply_gid, &center_id, GraphEdgeType::Replied, None);
        }
        for repost in reposts {
            let repost_gid = builder.push_authored_post(repost);
            builder.edge(&repost_gid, &center_id, GraphEdgeType::Reposted, None);
        }
        for parent in parents {
            let edge_type = match parent.rel.as_str() {
                "REPOSTED" => GraphEdgeType::Reposted,
                _ => GraphEdgeType::Replied,
            };
            let parent_gid = builder.push_authored_post(AuthoredPostRow {
                id: parent.id,
                author_id: parent.author_id,
                author_name: parent.author_name,
                author_image: parent.author_image,
                content: parent.content,
                kind: parent.kind,
                indexed_at: parent.indexed_at,
            });
            builder.edge(&center_id, &parent_gid, edge_type, None);
        }
        for mentioned in mentioned_nodes {
            let mentioned_gid = builder.push_user(&mentioned.id, mentioned.name, mentioned.image);
            builder.edge(&center_id, &mentioned_gid, GraphEdgeType::Mentioned, None);
        }
        for label in labels {
            let tag_id = builder.push_tag(&label.label, label.count);
            builder.edge(
                &tag_id,
                &center_id,
                GraphEdgeType::Tagged,
                Some(&label.label),
            );
        }

        Ok(Some(builder.build(kinds, &center_id, true)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_csv_parses() {
        let kinds: GraphKinds = serde_json::from_str("\"user,tag\"").unwrap();
        assert_eq!(
            kinds,
            GraphKinds {
                user: true,
                post: false,
                tag: true
            }
        );
    }

    #[test]
    fn kinds_tolerates_duplicates_and_whitespace() {
        let kinds: GraphKinds = serde_json::from_str("\"user, user\"").unwrap();
        assert_eq!(
            kinds,
            GraphKinds {
                user: true,
                post: false,
                tag: false
            }
        );
    }

    #[test]
    fn kinds_rejects_unknown() {
        assert!(serde_json::from_str::<GraphKinds>("\"user,banana\"").is_err());
    }

    #[test]
    fn depth_bounds_enforced() {
        assert_eq!(
            serde_json::from_str::<GraphDepth>("\"2\"").unwrap().get(),
            2
        );
        assert!(serde_json::from_str::<GraphDepth>("\"0\"").is_err());
        assert!(serde_json::from_str::<GraphDepth>("\"3\"").is_err());
        assert_eq!(GraphDepth::default().get(), 1);
    }
}
