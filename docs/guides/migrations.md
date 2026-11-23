# Data migrations

A change to the data Nexus has already stored in Neo4j or Redis ships as a migration: a step `nexusd` runs once against a deployment's databases. This guide covers running them, for operators, and writing one, for contributors.

## Running migrations

```bash
nexusd db migration check   # lists the pending migrations: exits 0 when none, 10 when some
nexusd db migration run     # runs them
```

Both read `migrations/config.toml` under the config folder, `~/.pubky-nexus/` or the one given with `--config-dir`, not the `config.toml` the API and the watcher use. It holds `backfill_ready` (below) and its own `[stack]`, with the database addresses. It is created from defaults on the first run, and those point at `localhost`: on a server, set its `[stack.db]` to the deployment's Neo4j and Redis before running anything.

- **When:** before starting a release that registers new migrations. `check` lists them; one that needs downtime says so in the release notes ([releasing.md](releasing.md)).
- **Nothing checks for you.** The API and the watcher start with migrations pending and may then serve wrong data, so run `check` as part of a deployment.
- **State:** each migration's progress is a `:Migration` node in Neo4j, with its `id` and `phase`. A migration whose node says `done` is not run again. A failed migration keeps its phase and is retried by the next `run`.
- **Order:** migrations run in the order they are registered, which is the order they were added.

## How a migration runs

A migration implements the `Migration` trait in `nexusd/src/migrations/manager.rs`. It is either single-stage or multi-stage, from `is_multi_staged()`.

**Single-stage.** The first `run` stores the migration at `backfill`, runs its `backfill()` and marks it `done`. Every migration registered today is single-stage: a backfill, a reindex, an index created again or a set of keys deleted.

**Multi-stage,** for moving data from an old place to a new one while Nexus keeps serving:

| Phase | What happens | How it moves on |
|---|---|---|
| `dual_write` | The application writes to the old and the new place. | When the new writes are stable, add the migration's id to `backfill_ready` in `migrations/config.toml`. The next `run` sets it to `backfill` and goes straight on to the next row. |
| `backfill` | `backfill()` copies the existing data to the new place. | In that same `run`: when `backfill()` succeeds, the migration moves to `cutover`. Remove its id from `backfill_ready` before the following `run`. |
| `cutover` | `cutover()` switches the reads to the new place. | The next `run`. |
| `cleanup` | `cleanup()` removes the old data. | The next `run`, which marks it `done`. |

Each `run` executes at most one phase of a migration, so a multi-stage migration takes several runs, with a deployment between them where the code changes.

> ⚠️ Remove a migration from `backfill_ready` right after the `run` that ran its backfill. Every `run` sets the migrations listed there back to `backfill` and runs the backfill again, even when they are at `cutover` or `done`, so a migration left on the list never runs its `cutover()`.

## Writing a migration

1. Create it:

   ```bash
   cargo run -p nexusd -- db migration new UserSearchStateBackfill
   ```

   Run it from the repository root: it writes `nexusd/src/migrations/catalog/user_search_state_backfill_<unix time>.rs` from a template and declares the module in `catalog/mod.rs`. The struct and its `id()` are the name in PascalCase with the time appended, such as `UserSearchStateBackfill1792157548`.
2. Register it: add `Box::new(YourMigration)` at the end of `import_migrations` in `nexusd/src/migrations/catalog/mod.rs`. A migration that must run after another goes after it; say why in a comment.
3. Implement it. The template is multi-stage. For a single-stage migration, return `false` from `is_multi_staged()`, write the work in `backfill()` and leave the other phases as `Ok(())`.
4. Make it safe to run again. A run that fails part-way is retried from the start of its phase, so a migration must give the same result when it runs twice.
5. Say what an operator needs to know in the file's doc comment and in the release's **Upgrade** notes: the order against other migrations, whether the watchers must be stopped, and whether an older release can still read the data afterwards.

The existing migrations in `nexusd/src/migrations/catalog/` are the examples to follow.
