//! The app's migration engine, ported from `pubky-social-specs/migration` (`pkg/MIGRATION.md`)
//! over the same transforms: the crate's [`MigrationCtx`], which the npm package compiles to
//! wasm. What it writes to the homeserver, and in which order, is what nexus sees of a
//! migration, so the port keeps the engine's walk:
//!
//! 1. Probe the flag `priv/social/v1/_migrated.json`; a flag at the current transform revision
//!    ends the run as `already_migrated` without listing anything.
//! 2. List `pub/social/v1/` and `priv/social/v1/`: the journal. A key present in either root is
//!    never written again, so an edit made after an earlier run survives. A post is keyed by
//!    its id whatever its version, media by its exact path, anything else by its path below the
//!    root.
//! 3. List `pub/pubky.app/` and walk it by pass: files, blobs, posts, tags, follows, the
//!    profile, then the private feeds, bookmarks and mutes.
//! 4. Each object: GET, transform, PUT every write whose key is absent, then HEAD the source;
//!    a source the owner deleted meanwhile takes the copies just written back out.
//! 5. Write the flag, unless an object hit an I/O error: the run then ends `incomplete`, and
//!    the next run walks again and copies what is still missing.
//!
//! Not ported: the capability check, the lock, the retries and the size bounds. The engine runs
//! two objects at a time; the port runs one, which only narrows the orders a test can see.
//! [`Migration::fail_reading`] makes one object's GET fail for good, so a test can end a run
//! `incomplete` and have the next one copy that object.

use std::collections::{BTreeMap, HashSet};

use anyhow::{bail, Context, Result};
use pubky::errors::RequestError;
use pubky::PubkySession;
use pubky_social_specs::migrate::{MigrationCtx, TRANSFORM_REV};
use pubky_social_specs::PubkyId;
use serde::{Deserialize, Serialize};

const V0_ROOT: &str = "pub/pubky.app/";
const V1_ROOTS: [&str; 2] = ["pub/social/v1/", "priv/social/v1/"];
const FLAG: &str = "priv/social/v1/_migrated.json";

/// One pass of the walk, named by the 0.x resource segment, in walk order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Pass {
    Files,
    Blobs,
    Posts,
    Tags,
    Follows,
    Profile,
    Feeds,
    Bookmarks,
    Mutes,
}

impl Pass {
    const ALL: [Pass; 9] = [
        Pass::Files,
        Pass::Blobs,
        Pass::Posts,
        Pass::Tags,
        Pass::Follows,
        Pass::Profile,
        Pass::Feeds,
        Pass::Bookmarks,
        Pass::Mutes,
    ];

    /// The pass of a path relative to the 0.x namespace (`posts/X`, `profile.json`), `None`
    /// for one no pass migrates, such as `settings.json` and `last_read`.
    fn of(legacy_relative: &str) -> Option<Pass> {
        if legacy_relative == "profile.json" {
            return Some(Pass::Profile);
        }
        let (segment, rest) = legacy_relative.split_once('/')?;
        if rest.is_empty() {
            return None;
        }
        Some(match segment {
            "files" => Pass::Files,
            "blobs" => Pass::Blobs,
            "posts" => Pass::Posts,
            "tags" => Pass::Tags,
            "follows" => Pass::Follows,
            "feeds" => Pass::Feeds,
            "bookmarks" => Pass::Bookmarks,
            "mutes" => Pass::Mutes,
            _ => return None,
        })
    }
}

/// What a run did, by outcome, as owner-relative paths without the leading slash.
#[derive(Debug, Default)]
pub struct Report {
    pub already_migrated: bool,
    /// The `social/v1` paths written.
    pub written: Vec<String>,
    /// 0.x paths whose every write was already present.
    pub already_present: Vec<String>,
    /// 0.x paths the transforms skipped, with the category.
    pub skipped: Vec<(String, &'static str)>,
    /// 0.x paths gone before or right after their copy.
    pub deleted_mid_run: Vec<String>,
    /// 0.x paths no pass takes.
    pub not_migrated: Vec<String>,
    /// 0.x paths whose GET failed for good: the run ends `incomplete`, without a flag.
    pub io_error: Vec<String>,
}

impl Report {
    /// Whether the run ended `incomplete`, to be run again.
    pub fn incomplete(&self) -> bool {
        !self.io_error.is_empty()
    }
}

/// The flag a finished run leaves, as the engine writes it.
#[derive(Debug, Serialize, Deserialize)]
struct Flag {
    migrated_at: i64,
    transform_rev: u32,
    skipped: BTreeMap<String, Vec<String>>,
    migrated: Vec<String>,
}

/// A run in progress over one owner's tree.
pub struct Migration<'a> {
    session: &'a PubkySession,
    ctx: MigrationCtx,
    /// The journal: keys of what the `social/v1` roots hold.
    present: HashSet<String>,
    /// 0.x paths a finished earlier run migrated: never copied again.
    migrated_before: HashSet<String>,
    /// The listed 0.x tree, by pass, not yet walked.
    todo: BTreeMap<Pass, Vec<String>>,
    migrated: Vec<String>,
    /// 0.x paths whose GET fails, as a homeserver that stops answering for one does.
    failing: HashSet<String>,
    pub report: Report,
}

impl<'a> Migration<'a> {
    /// Probes the flag and lists both trees. A tree the flag records as migrated at the current
    /// revision lists nothing and walks nothing.
    pub async fn start(session: &'a PubkySession, owner: &PubkyId) -> Result<Self> {
        let mut migration = Self {
            session,
            ctx: MigrationCtx::new(owner.clone()),
            present: HashSet::new(),
            migrated_before: HashSet::new(),
            todo: BTreeMap::new(),
            migrated: Vec::new(),
            failing: HashSet::new(),
            report: Report::default(),
        };
        if let Some(flag) = migration.get(FLAG).await? {
            let flag: Flag = serde_json::from_slice(&flag).context("reading the flag")?;
            if flag.transform_rev >= TRANSFORM_REV {
                migration.report.already_migrated = true;
                return Ok(migration);
            }
            migration.migrated_before = flag.migrated.into_iter().collect();
        }
        for root in V1_ROOTS {
            for path in migration.list(root).await? {
                migration.present.insert(journal_key(&path));
            }
        }
        for path in migration.list(V0_ROOT).await? {
            let relative = path.strip_prefix(V0_ROOT).unwrap_or(&path);
            match Pass::of(relative) {
                Some(pass) => migration.todo.entry(pass).or_default().push(path),
                None => migration.report.not_migrated.push(path),
            }
        }
        Ok(migration)
    }

    /// Walks every pass up to and including `last` that has not run yet.
    async fn run_through(&mut self, last: Pass) -> Result<()> {
        for pass in Pass::ALL.into_iter().filter(|pass| *pass <= last) {
            for path in self.todo.remove(&pass).unwrap_or_default() {
                self.object(pass, &path).await?;
            }
        }
        Ok(())
    }

    /// Makes every GET of the 0.x object at `path` fail, past the engine's retries: the object
    /// counts `io_error` and the run ends `incomplete`.
    pub fn fail_reading(&mut self, path: &str) {
        self.failing
            .insert(path.trim_start_matches('/').to_string());
    }

    /// Walks what is left and writes the flag, unless an object hit an I/O error: that run ends
    /// `incomplete` and the next one walks again. Returns the report.
    pub async fn finish(mut self) -> Result<Report> {
        if self.report.already_migrated {
            return Ok(self.report);
        }
        self.run_through(Pass::Mutes).await?;
        if self.report.incomplete() {
            return Ok(self.report);
        }
        let mut skipped: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (path, category) in &self.report.skipped {
            skipped
                .entry(category.to_string())
                .or_default()
                .push(path.clone());
        }
        if !self.report.deleted_mid_run.is_empty() {
            skipped.insert(
                "deleted_mid_run".to_string(),
                self.report.deleted_mid_run.clone(),
            );
        }
        let flag = Flag {
            migrated_at: chrono::Utc::now().timestamp_micros(),
            transform_rev: TRANSFORM_REV,
            skipped,
            migrated: std::mem::take(&mut self.migrated),
        };
        self.put(FLAG, serde_json::to_vec(&flag)?).await?;
        Ok(self.report)
    }

    /// One 0.x object of `pass` through its own step. A File object writes nothing and feeds the
    /// run, so it is read on every walk, even when an earlier run migrated it, and is not
    /// recorded as migrated; everything else an earlier run migrated is present.
    async fn object(&mut self, pass: Pass, path: &str) -> Result<()> {
        if pass != Pass::Files && self.migrated_before.contains(path) {
            self.report.already_present.push(path.to_string());
            self.migrated.push(path.to_string());
            return Ok(());
        }
        if self.failing.contains(path) {
            self.report.io_error.push(path.to_string());
            return Ok(());
        }
        let Some(bytes) = self.get(path).await? else {
            self.report.deleted_mid_run.push(path.to_string());
            return Ok(());
        };
        let migrated = match self.ctx.migrate(path, &bytes) {
            Ok(migrated) => migrated,
            Err(skipped) => {
                self.report
                    .skipped
                    .push((path.to_string(), skipped.skip.as_str()));
                return Ok(());
            }
        };
        // A File object feeds the run and writes nothing
        if pass == Pass::Files {
            return Ok(());
        }
        let mut written = Vec::new();
        for (target, body) in migrated.writes {
            let key = journal_key(&target);
            if self.present.contains(&key) {
                continue;
            }
            self.put(&target, body).await?;
            self.present.insert(key);
            written.push(target);
        }
        if !self.exists(path).await? {
            for target in &written {
                self.delete(target).await?;
            }
            self.report.deleted_mid_run.push(path.to_string());
            return Ok(());
        }
        if written.is_empty() {
            self.report.already_present.push(path.to_string());
        }
        self.report.written.extend(written);
        self.migrated.push(path.to_string());
        Ok(())
    }

    /// Every path under `root`, deep and ascending, without the leading slash. A root the
    /// homeserver does not have is empty.
    async fn list(&self, root: &str) -> Result<Vec<String>> {
        let mut paths = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let storage = self.session.storage();
            let mut builder = storage.list(format!("/{root}"))?.limit(1000);
            if let Some(cursor) = &cursor {
                builder = builder.cursor(cursor);
            }
            let page = match builder.send().await {
                Ok(page) => page,
                Err(e) if is_not_found(&e) => break,
                Err(e) => return Err(e).with_context(|| format!("listing {root}")),
            };
            let Some(last) = page.last() else { break };
            cursor = Some(last.to_pubky_url());
            paths.extend(
                page.iter()
                    .map(|resource| resource.path.as_str().trim_start_matches('/').to_string())
                    .filter(|path| !path.ends_with('/')),
            );
        }
        Ok(paths)
    }

    async fn get(&self, path: &str) -> Result<Option<Vec<u8>>> {
        match self.session.storage().get(format!("/{path}")).await {
            Ok(response) => Ok(Some(response.bytes().await?.to_vec())),
            Err(e) if is_not_found(&e) => Ok(None),
            Err(e) => Err(e).with_context(|| format!("GET {path}")),
        }
    }

    async fn exists(&self, path: &str) -> Result<bool> {
        Ok(self.session.storage().exists(format!("/{path}")).await?)
    }

    async fn put(&self, path: &str, body: Vec<u8>) -> Result<()> {
        self.session
            .storage()
            .put(format!("/{path}"), body)
            .await
            .with_context(|| format!("PUT {path}"))?;
        Ok(())
    }

    async fn delete(&self, path: &str) -> Result<()> {
        match self.session.storage().delete(format!("/{path}")).await {
            Ok(_) => Ok(()),
            Err(e) if is_not_found(&e) => Ok(()),
            Err(e) => Err(e).with_context(|| format!("DELETE {path}")),
        }
    }
}

/// Runs the whole migration of `owner`'s tree through `session`.
pub async fn migrate(session: &PubkySession, owner: &PubkyId) -> Result<Report> {
    Migration::start(session, owner).await?.finish().await
}

/// The journal key of a `social/v1` path: a post by its id, whatever its version and root;
/// media by its exact path, since a private copy does not serve a public reference; anything
/// else by its path below the root, so a private copy counts.
fn journal_key(path: &str) -> String {
    let Some((root, rest)) = V1_ROOTS
        .iter()
        .find_map(|root| path.strip_prefix(root).map(|rest| (*root, rest)))
    else {
        return path.to_string();
    };
    if let Some(post) = rest.strip_prefix("posts/") {
        let id = post.split('/').next().unwrap_or(post);
        return format!("posts/{id}");
    }
    if rest.starts_with("files/") {
        return format!("{root}{rest}");
    }
    rest.to_string()
}

fn is_not_found(error: &pubky::Error) -> bool {
    matches!(
        error,
        pubky::Error::Request(RequestError::Server { status, .. })
            if status.as_u16() == 404 || status.as_u16() == 410
    )
}

/// Fails unless `report` wrote something and skipped nothing: the fixtures write only objects
/// the 0.x reader accepts.
pub fn ensure_clean(report: &Report) -> Result<()> {
    if report.written.is_empty()
        || !report.skipped.is_empty()
        || !report.deleted_mid_run.is_empty()
        || report.incomplete()
    {
        bail!("the migration left objects behind: {report:?}");
    }
    Ok(())
}
