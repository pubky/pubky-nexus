//! The agreement with the migrator: the addresses and tag ids the migration derives from the
//! graph equal the ones `migrate::transform` writes from the same v0 bytes, so a change in the
//! specs crate can't silently split them. The migrator is a dev-dependency feature, and nothing
//! it produces is stored.
//!
//! Each side gets the author's bytes. The migrator reads them through the frozen 0.x reader and
//! writes the v1 file. Nexus reads them through the same reader, as the watcher did when it
//! indexed them, and hands what the graph would hold (a `File` node's `src` and content type, a
//! post's attachments, a tag's id, label and target as the router classifies it) to the
//! migration's pure functions. Two mix-ups the test has to catch: a v1 builder swapped for its
//! `legacy_v0` namesake, and a drift in the attachment function's copy of the migrator's
//! private `fold_scheme`.

use super::attachments;
use super::files::FileMap;
use super::tags::{self, StoredTag, Target};
use crate::events::inputs::ReferenceTarget;
use crate::events::route::resolve_reference;
use nexus_common::universal_tag::normalize::normalize_uri;
use pubky_social_specs::legacy_v0::traits::{HashId as V0HashId, Validatable as V0Validatable};
use pubky_social_specs::legacy_v0::{PubkyAppFile, PubkyAppPost, PubkyAppTag};
use pubky_social_specs::migrate::{self, MigrationCtx, Skip};
use pubky_social_specs::{PubkyId, PubkySocialArticleContent, PubkySocialPost, PROTOCOL};
use serde_json::json;

const OWNER: &str = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
const OTHER: &str = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";

/// The owner's blob two files name: an image under the lowest id, a PDF under the next.
const IMAGE_HASH: &str = "8Z8CWH8NVYQY39ZEBFGKQWWEKG";
/// The other user's blob, which one of the owner's files names.
const OTHERS_HASH: &str = "5CNJPASB5CNJPASB5CNJPASB5C";
/// The owner's blob one text file names.
const TEXT_HASH: &str = "AKSZ57W2RFKHV1EHK007FQQ8TW";
/// An owner's blob no file names.
const UNNAMED_HASH: &str = "0PKGB9R5MW2TE1D70PKGB9R5MW";

const LABEL: &str = "nexus";
const CREATED_AT: i64 = 1_750_000_000_000;

/// A v0 timestamp id `n` microseconds after a fixed instant in 2025, so ids order by `n`.
fn tsid(n: i64) -> String {
    const BASE_MICROS: i64 = 1_750_000_000_000_000;
    base32::encode(
        base32::Alphabet::Crockford,
        &(BASE_MICROS + n).to_be_bytes(),
    )
}

fn v0(user: &str, path: &str) -> String {
    format!("{PROTOCOL}{user}/pub/pubky.app/{path}")
}

fn blob(user: &str, hash: &str) -> String {
    v0(user, &format!("blobs/{hash}"))
}

fn file(user: &str, n: i64) -> String {
    v0(user, &format!("files/{}", tsid(n)))
}

fn post(user: &str, n: i64) -> String {
    v0(user, &format!("posts/{}", tsid(n)))
}

/// The owner's v0 `File` objects, as `(tsid, src, content_type)`: the two namers of the image
/// blob with the higher id first, a `src` that is another user's blob, a `src` that is the
/// owner's but no blob, and a text file.
fn v0_files() -> Vec<(String, String, String)> {
    [
        (2, blob(OWNER, IMAGE_HASH), "application/pdf"),
        (1, blob(OWNER, IMAGE_HASH), "image/png"),
        (3, blob(OTHER, OTHERS_HASH), "image/png"),
        (4, file(OWNER, 1), "image/png"),
        (5, blob(OWNER, TEXT_HASH), "text/plain"),
    ]
    .into_iter()
    .map(|(n, src, content_type)| (tsid(n), src, content_type.to_string()))
    .collect()
}

fn file_bytes(src: &str, content_type: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "name": "photo",
        "created_at": CREATED_AT,
        "src": src,
        "content_type": content_type,
        "size": 20,
    }))
    .unwrap()
}

/// Both sides, fed the same `File` objects: the migrator reads them into its run, and Nexus
/// builds its map from what the v0 reader stored for each node.
fn both_sides() -> (PubkyId, FileMap, MigrationCtx) {
    let owner = PubkyId::try_from(OWNER).unwrap();
    let mut ctx = MigrationCtx::new(owner.clone());
    let mut nodes = vec![];
    for (tsid, src, content_type) in v0_files() {
        let bytes = file_bytes(&src, &content_type);
        ctx.read_v0_file(&tsid, &bytes)
            .unwrap_or_else(|e| panic!("the migrator refused file {tsid}: {e}"));
        let stored: PubkyAppFile = V0Validatable::try_from(&bytes, &tsid)
            .unwrap_or_else(|e| panic!("the v0 reader refused file {tsid}: {e}"));
        nodes.push((tsid, stored.src, stored.content_type));
    }
    let files = FileMap::from_files(&owner, nodes);
    (owner, files, ctx)
}

fn post_bytes(kind: &str, content: &str, attachments: &[&str]) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "content": content,
        "kind": kind,
        "parent": null,
        "embed": null,
        "attachments": attachments,
    }))
    .unwrap()
}

/// The one file the migrator writes for the v0 object at `path`: its path and bytes.
fn migrated(ctx: &MigrationCtx, path: &str, bytes: &[u8]) -> (String, Vec<u8>) {
    let migrated = migrate::transform(path, bytes, ctx)
        .unwrap_or_else(|e| panic!("the migrator skipped {path}: {e}"));
    let [write] = migrated.writes.as_slice() else {
        panic!("one write expected, got {:?}", migrated.writes);
    };
    write.clone()
}

/// Why the migrator writes nothing for the v0 object at `path`.
fn skip(ctx: &MigrationCtx, path: &str, bytes: &[u8]) -> Skip {
    migrate::transform(path, bytes, ctx).unwrap_err().skip
}

/// The post the migrator writes for the v0 post `id`.
fn migrated_post(ctx: &MigrationCtx, id: &str, bytes: &[u8]) -> PubkySocialPost {
    let (path, bytes) = migrated(ctx, &format!("pub/pubky.app/posts/{id}"), bytes);
    assert_eq!(path, format!("pub/social/v1/posts/{id}/{id}.json"));
    serde_json::from_slice(&bytes).unwrap()
}

/// The attachments Nexus holds for the v0 post `id`, read as the watcher read them, after the
/// migration's rewrite.
fn nexus_attachments(owner: &PubkyId, files: &FileMap, id: &str, bytes: &[u8]) -> Vec<String> {
    let stored: PubkyAppPost = V0Validatable::try_from(bytes, id)
        .unwrap_or_else(|e| panic!("the v0 reader refused post {id}: {e}"));
    stored
        .attachments
        .unwrap_or_default()
        .iter()
        .map(|uri| attachments::rewrite(owner, files, uri))
        .collect()
}

fn uris(post: &PubkySocialPost) -> Vec<String> {
    post.attachments.iter().map(|a| a.uri.clone()).collect()
}

/// The v0 tag on `target` with `label`, as the author wrote it: its id, as the client computed
/// it, and its bytes.
fn v0_tag(target: &str, label: &str) -> (String, Vec<u8>) {
    let tag = PubkyAppTag {
        uri: target.to_string(),
        label: label.to_string(),
        created_at: CREATED_AT,
    };
    let bytes = serde_json::to_vec(&tag).unwrap();
    (tag.sanitize().create_id(), bytes)
}

/// The edge Nexus holds for the v0 tag `id`, read and routed as the watcher did; `None` when
/// the watcher indexes no edge for that target.
fn stored_tag(id: &str, bytes: &[u8]) -> Option<StoredTag> {
    let stored: PubkyAppTag = V0Validatable::try_from(bytes, id)
        .unwrap_or_else(|e| panic!("the v0 reader refused tag {id}: {e}"));
    let target = match resolve_reference(&stored.uri).target {
        ReferenceTarget::Post(key) => Target::Post {
            author_id: Some(key.owner_id.to_string()),
            post_id: key.resource_id,
        },
        ReferenceTarget::User(user_id) => Target::User(user_id.to_string()),
        ReferenceTarget::File(_) | ReferenceTarget::Foreign => {
            Target::Resource(normalize_uri(&stored.uri).unwrap().0)
        }
        ReferenceTarget::Tag { .. } | ReferenceTarget::Other => return None,
    };
    Some(StoredTag {
        edge: "4:edge:1".to_string(),
        id: id.to_string(),
        label: stored.label,
        target,
    })
}

/// The id and the address of the tag file the migrator writes for the v0 tag `id`.
fn migrated_tag(ctx: &MigrationCtx, id: &str, bytes: &[u8]) -> (String, String) {
    let (path, _) = migrated(ctx, &format!("pub/pubky.app/tags/{id}"), bytes);
    let new_id = path
        .strip_prefix("pub/social/v1/tags/")
        .and_then(|rest| rest.strip_suffix(".json"))
        .unwrap_or_else(|| panic!("not a v1 tag path: {path}"));
    (new_id.to_string(), format!("{PROTOCOL}{OWNER}/{path}"))
}

/// A `File` whose `src` is not the owner's blob names nothing on either side: a reference
/// through it keeps its address, and the blob it named is not in the extension table. Every
/// blob the two sides do know migrates under the same extension, the lowest id's.
#[test]
fn a_file_whose_src_is_not_the_owners_blob_is_out_of_the_map_on_both_sides() {
    let (owner, files, ctx) = both_sides();

    for n in [3, 4] {
        assert_eq!(files.hash_of(&tsid(n)), None, "file {n}");
        let uri = file(OWNER, n);
        let post_id = tsid(100 + n);
        let bytes = post_bytes("short", "x", &[&uri]);
        assert_eq!(
            nexus_attachments(&owner, &files, &post_id, &bytes),
            vec![uri.clone()],
            "file {n}"
        );
        assert_eq!(
            uris(&migrated_post(&ctx, &post_id, &bytes)),
            vec![uri],
            "file {n}"
        );
    }

    for (hash, ext) in [
        (IMAGE_HASH, "png"),
        (TEXT_HASH, "txt"),
        (OTHERS_HASH, "bin"),
        (UNNAMED_HASH, "bin"),
    ] {
        assert_eq!(files.ext_of(hash), ext, "{hash}");
        let path = ctx
            .blob_destination(&format!("pub/pubky.app/blobs/{hash}"), 20, hash)
            .unwrap();
        assert_eq!(
            attachments::rewrite(&owner, &files, &blob(OWNER, hash)),
            format!("{PROTOCOL}{OWNER}/{path}"),
            "{hash}"
        );
    }
}

/// Every attachment Nexus rewrites takes the migrator's address, and every one it keeps is
/// kept by the migrator too. One post per spelling, so a disagreement names it.
#[test]
fn every_attachment_takes_the_migrators_address() {
    let (owner, files, ctx) = both_sides();

    let rewritten = [
        file(OWNER, 1),
        file(OWNER, 2),
        file(OWNER, 5),
        format!("  {} \n", file(OWNER, 1)),
        blob(OWNER, IMAGE_HASH),
        blob(OWNER, UNNAMED_HASH),
        post(OWNER, 50),
        post(OTHER, 51),
        format!("PUBKY://{OWNER}/pub/pubky.app/posts/{}", tsid(50)),
        v0(OWNER, "profile.json"),
        v0(OTHER, "profile.json"),
        v0(OWNER, &format!("follows/{OTHER}")),
    ];
    let kept = [
        file(OWNER, 3),
        file(OWNER, 4),
        file(OWNER, 9),
        file(OTHER, 1),
        blob(OTHER, IMAGE_HASH),
        v0(OWNER, "tags/0032SSN7Q4EVG"),
        v0(OWNER, "feeds/0032SSN7Q4EVG"),
        v0(OWNER, "bookmarks/0032SSN7Q4EVG"),
        v0(OWNER, &format!("mutes/{OTHER}")),
        v0(OWNER, "last_read"),
        format!("{PROTOCOL}{OWNER}"),
        format!("{PROTOCOL}{OWNER}/pub/other.app/items/1"),
        format!("{PROTOCOL}{OWNER}/pub/social/v1/files/{IMAGE_HASH}.png"),
        "https://example.com/a.png?x=1#top".to_string(),
        "https://user:pw@example.com/a.png".to_string(),
        "https://example.com/a/./b/../c.png".to_string(),
        "https://example.com/a.png?".to_string(),
        "https://example.com/a%20b.png".to_string(),
        "https://example.com/a b.png".to_string(),
        "HTTPS://EXAMPLE.COM/A.png".to_string(),
        "http://example.com:80/a.png".to_string(),
    ];

    for (n, uri) in rewritten.iter().chain(&kept).enumerate() {
        let post_id = tsid(200 + n as i64);
        let bytes = post_bytes("short", "x", &[uri]);
        let stored: PubkyAppPost = V0Validatable::try_from(&bytes, &post_id).unwrap();
        let stored = stored.attachments.unwrap()[0].clone();
        let ours = nexus_attachments(&owner, &files, &post_id, &bytes);
        let theirs = uris(&migrated_post(&ctx, &post_id, &bytes));
        assert_eq!(ours, theirs, "{uri:?}");
        if n < rewritten.len() {
            assert_ne!(ours[0], stored, "{uri:?} should be rewritten");
        } else {
            assert_eq!(ours[0], stored, "{uri:?} should be kept");
        }
    }

    // Order and count are kept across one post
    let post_id = tsid(300);
    let all: Vec<&str> = rewritten.iter().chain(&kept).map(String::as_str).collect();
    let bytes = post_bytes("short", "x", &all[..10]);
    assert_eq!(
        nexus_attachments(&owner, &files, &post_id, &bytes),
        uris(&migrated_post(&ctx, &post_id, &bytes))
    );
}

/// The migrator moves an image first attachment of a titled article into `cover_image`, and
/// Nexus keeps it, rewritten, at the head of `attachments`: the two addresses are the same, and
/// the attachments after it are the same. A first attachment that is not an image stays one.
#[test]
fn an_articles_moved_cover_is_the_attachment_nexus_keeps() {
    let (owner, files, ctx) = both_sides();

    let post_id = tsid(400);
    let bytes = post_bytes(
        "long",
        r#"{"title":"Title","body":"Body"}"#,
        &[
            &file(OWNER, 1),
            &file(OWNER, 5),
            "https://example.com/a.png",
        ],
    );
    let migrated = migrated_post(&ctx, &post_id, &bytes);
    let article: PubkySocialArticleContent = serde_json::from_str(&migrated.content).unwrap();
    let cover = article
        .cover_image
        .expect("an image first attachment moves into the cover");
    let mut theirs = vec![cover];
    theirs.extend(uris(&migrated));
    let ours = nexus_attachments(&owner, &files, &post_id, &bytes);
    assert_eq!(ours.len(), 3);
    assert_eq!(ours, theirs);

    // A title in the first body line, and a text file first: nothing moves
    let post_id = tsid(401);
    let bytes = post_bytes(
        "long",
        "Title line\nBody",
        &[&file(OWNER, 5), &file(OWNER, 1)],
    );
    let migrated = migrated_post(&ctx, &post_id, &bytes);
    let article: PubkySocialArticleContent = serde_json::from_str(&migrated.content).unwrap();
    assert_eq!(article.cover_image, None);
    assert_eq!(
        nexus_attachments(&owner, &files, &post_id, &bytes),
        uris(&migrated)
    );
}

/// The one reference the migrator rewrites as the author spelled it is an article envelope's
/// cover: the 0.x reader re-serializes every attachment and tag target through `Url`, which
/// already lowercases the scheme, but keeps `content` whole. So the cover is where the
/// attachment function's copy of the migrator's private `fold_scheme` meets the original.
#[test]
fn the_scheme_fold_matches_the_migrators_on_a_cover_kept_as_written() {
    let (owner, files, ctx) = both_sides();
    let covers = [
        format!("PUBKY://{OWNER}/pub/pubky.app/files/{}", tsid(1)),
        format!("Pubky://{OWNER}/pub/pubky.app/posts/{}", tsid(50)),
        format!("PUBKY://{OTHER}/pub/pubky.app/profile.json"),
        format!("pubky{OWNER}/pub/pubky.app/blobs/{IMAGE_HASH}"),
        "HTTPS://Example.com/Cover.png".to_string(),
    ];
    for (n, cover) in covers.iter().enumerate() {
        let post_id = tsid(500 + n as i64);
        let content = json!({"title": "Title", "body": "Body", "cover_image": cover}).to_string();
        let bytes = post_bytes("long", &content, &[]);
        let migrated = migrated_post(&ctx, &post_id, &bytes);
        let article: PubkySocialArticleContent = serde_json::from_str(&migrated.content).unwrap();
        assert_eq!(
            Some(attachments::rewrite(&owner, &files, cover)),
            article.cover_image,
            "{cover}"
        );
        // The fold is not a no-op here
        assert_ne!(article.cover_image.as_deref(), Some(cover.as_str()));
    }
}

/// Each row of the tag table: the id and the address Nexus gives an edge equal the id and the
/// path of the file the migrator writes for the same tag. A computed row changes the id; a
/// kept row keeps it, because the migrator keeps the target too.
#[test]
fn every_tag_row_takes_the_migrators_id_and_address() {
    let (owner, files, ctx) = both_sides();

    let computed = [
        (post(OWNER, 50), LABEL),
        (post(OTHER, 51), LABEL),
        (
            format!("PUBKY://{OTHER}/pub/pubky.app/posts/{}", tsid(51)),
            LABEL,
        ),
        // A loose spelling the v0 reader and the migrator both read as the post
        (format!("{}/extra", post(OTHER, 51)), LABEL),
        (v0(OWNER, "profile.json"), LABEL),
        (v0(OTHER, "profile.json"), LABEL),
        (v0(OTHER, "profile.json"), "  Nexus "),
        (file(OWNER, 1), LABEL),
        (file(OWNER, 2), LABEL),
        (file(OWNER, 5), LABEL),
    ];
    let kept = [
        // Bare keys
        (format!("{PROTOCOL}{OTHER}"), LABEL),
        (format!("{PROTOCOL}{OWNER}"), LABEL),
        // The owner's files out of the map, an own file with no File object, another user's
        (file(OWNER, 3), LABEL),
        (file(OWNER, 4), LABEL),
        (file(OWNER, 9), LABEL),
        (file(OTHER, 1), LABEL),
        // Web URLs
        ("https://example.com/page#section".to_string(), LABEL),
        ("https://user:pw@example.com/page".to_string(), LABEL),
        ("https://example.com/a/./b/../c".to_string(), LABEL),
        ("https://example.com/page?".to_string(), LABEL),
        ("https://example.com/a%20b".to_string(), LABEL),
        ("https://example.com/a b".to_string(), LABEL),
        ("HTTPS://EXAMPLE.COM/Page".to_string(), LABEL),
        ("http://example.com:80/page".to_string(), LABEL),
        // Other schemes
        ("nostr:npub1abc".to_string(), LABEL),
        ("NOSTR:npub1abc".to_string(), LABEL),
        (
            "ipfs://bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi".to_string(),
            LABEL,
        ),
        ("geo:37.78,-122.4".to_string(), LABEL),
        ("magnet:?xt=urn:btih:abc".to_string(), LABEL),
        (
            "did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK".to_string(),
            LABEL,
        ),
        // Other apps' paths
        (format!("{PROTOCOL}{OWNER}/pub/other.app/items/1"), LABEL),
        (format!("{PROTOCOL}{OTHER}/pub/other.app/items/1"), LABEL),
        (
            format!("{PROTOCOL}{OWNER}/pub/eventky.app/events/abc"),
            LABEL,
        ),
        // v1 addresses written before the flip: a file, a versionless post
        (
            format!("{PROTOCOL}{OWNER}/pub/social/v1/files/{IMAGE_HASH}.png"),
            LABEL,
        ),
        (
            format!("{PROTOCOL}{OTHER}/pub/social/v1/posts/{}", tsid(51)),
            LABEL,
        ),
    ];

    for (n, (target, label)) in computed.iter().chain(&kept).enumerate() {
        let (id, bytes) = v0_tag(target, label);
        let stored = stored_tag(&id, &bytes).unwrap_or_else(|| panic!("no edge for {target:?}"));
        let ours = tags::rewrite(&owner, &files, &stored);
        let theirs = migrated_tag(&ctx, &id, &bytes);
        assert_eq!(ours, theirs, "{target:?} {label:?}");
        if n < computed.len() {
            assert_ne!(ours.0, id, "{target:?} should take a new id");
        } else {
            assert_eq!(ours.0, id, "{target:?} should keep its id");
        }
    }
}

/// The rows the migrator writes no file for, or has more than Nexus for. A tag whose id no
/// spelling reproduces fails the v0 reader, so the migrator skips it; Nexus keeps the id. A
/// pubky target the v1 reader calls non-canonical, such as a path with a query, makes the
/// migrator skip the tag, or the post carrying it as an attachment, whole; Nexus keeps the id
/// and the address, the spec's accepted limit of a tag with no v1 file. A tag on a `social/v1`
/// post version is refused by the v1 reader too ("must be versionless"), so the migrator skips
/// it; Nexus gives the edge the id of the versionless v1 URI, the one a real v1 tag on that
/// post carries. A tag on an own blob or on a tag is indexed by neither. A tag on a post with
/// no `AUTHORED` author keeps its id by decision, while the migrator, which reads the author
/// from the address, computes it: the one row where the two part, as the table says.
#[test]
fn the_rows_the_migrator_has_no_counterpart_for() {
    let (owner, files, ctx) = both_sides();

    let versionless = format!("{PROTOCOL}{OTHER}/pub/social/v1/posts/{}", tsid(51));
    let versioned = format!("{versionless}/{}.json", tsid(52));
    let (id, bytes) = v0_tag(&versioned, LABEL);
    assert_eq!(
        skip(&ctx, &format!("pub/pubky.app/tags/{id}"), &bytes),
        Skip::Invalid
    );
    let stored = stored_tag(&id, &bytes).unwrap();
    assert_eq!(
        stored.target,
        Target::Post {
            author_id: Some(OTHER.to_string()),
            post_id: tsid(51),
        }
    );
    let (versionless_id, versionless_bytes) = v0_tag(&versionless, LABEL);
    let (real_v1_id, _) = migrated_tag(&ctx, &versionless_id, &versionless_bytes);
    assert_ne!(real_v1_id, id);
    assert_eq!(tags::rewrite(&owner, &files, &stored).0, real_v1_id);

    let with_query = format!("{PROTOCOL}{OTHER}/pub/other.app/items/1?x=1");
    let (id, bytes) = v0_tag(&with_query, LABEL);
    assert_eq!(
        skip(&ctx, &format!("pub/pubky.app/tags/{id}"), &bytes),
        Skip::Invalid
    );
    let stored = stored_tag(&id, &bytes).unwrap();
    assert_eq!(stored.target, Target::Resource(with_query.clone()));
    assert_eq!(tags::rewrite(&owner, &files, &stored).0, id);
    let post_id = tsid(600);
    let bytes = post_bytes("short", "x", &[&with_query]);
    assert_eq!(
        skip(&ctx, &format!("pub/pubky.app/posts/{post_id}"), &bytes),
        Skip::Invalid
    );
    assert_eq!(
        nexus_attachments(&owner, &files, &post_id, &bytes),
        vec![with_query]
    );

    let (_, bytes) = v0_tag(&v0(OTHER, "profile.json"), LABEL);
    assert_eq!(
        skip(&ctx, "pub/pubky.app/tags/MADEUPTAGID", &bytes),
        Skip::Invalid
    );
    let unmatched = StoredTag {
        edge: "4:edge:1".to_string(),
        id: "MADEUPTAGID".to_string(),
        label: LABEL.to_string(),
        target: Target::User(OTHER.to_string()),
    };
    assert_eq!(
        tags::rewrite(&owner, &files, &unmatched),
        (
            "MADEUPTAGID".to_string(),
            format!("{PROTOCOL}{OWNER}/pub/social/v1/tags/MADEUPTAGID.json")
        )
    );

    for target in [blob(OWNER, IMAGE_HASH), v0(OWNER, "tags/0032SSN7Q4EVG")] {
        let (id, bytes) = v0_tag(&target, LABEL);
        assert_eq!(stored_tag(&id, &bytes), None, "{target}");
    }

    let (id, bytes) = v0_tag(&post(OWNER, 50), LABEL);
    let no_author = StoredTag {
        target: Target::Post {
            author_id: None,
            post_id: tsid(50),
        },
        ..stored_tag(&id, &bytes).unwrap()
    };
    let (ours, _) = tags::rewrite(&owner, &files, &no_author);
    let (theirs, _) = migrated_tag(&ctx, &id, &bytes);
    assert_eq!(ours, id);
    assert_ne!(ours, theirs);
}
