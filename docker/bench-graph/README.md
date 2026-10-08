# Graph query performance audit

A small harness to find graph queries whose cost grows faster than the data,
the class of bug behind [#935](https://github.com/pubky/pubky-nexus/issues/935)
(`user_counts` was O(received_tags x authored_posts)).

It seeds a synthetic graph at two scales, runs every audited query under
Neo4j `PROFILE`, and reports `Total database accesses` (db hits) per query. A
query whose db hits grow much faster than the 10x data growth is doing work
proportional to a *product* of dimensions, and gets flagged.

## How it fits together

| Piece | What it does |
|---|---|
| `seed.sh` | Builds a synthetic graph. Everything is labeled `:Bench` with `audit_*` ids, so cleanup is one `DETACH DELETE` and real data is never touched. |
| `examples/graph_audit` (Rust bin `graph-audit`) | Prints `PROFILE <cypher>` for each audited query using the **real** query functions via `Query::to_cypher_populated()`, so the profiled Cypher cannot drift from production. |
| `run.sh` | Seeds small then large, profiles every query, and writes a markdown table sorted worst-first. |

## Run it

Prereqs: the dockerised Neo4j running (`cd docker && docker compose up -d`),
nothing else seeded. Then from the repo root:

```sh
./docker/bench-graph/run.sh            # writes docker/bench-graph/report.md
```

Re-seed or clean by hand if needed:

```sh
./docker/bench-graph/seed.sh large     # whale-scale
./docker/bench-graph/seed.sh clean      # remove all :Bench data
FOLLOWERS=20000 ./docker/bench-graph/seed.sh large   # override one dimension
```

## Reading the report

```
| query           | dbhits S | dbhits L | growth | exec_ms L | flag       |
| get_global_...  |  3105439 |300472639 |  96.8x |    119318 | **REVIEW** |
```

- **growth** = dbhits(large) / dbhits(small). Large is 10x small, so:
  - **~1x** flat: the query does not depend on the scaled dimension (or hits a count-store fast path).
  - **~10x** linear: healthy, cost tracks the data.
  - **>10x** super-linear: cost tracks a *product* of dimensions, flagged `REVIEW`.
- **db hits are ground truth**; `exec_ms` is informational (machine/cache dependent).
- Always read growth *and* absolute db hits: a 200x growth off a tiny baseline can matter less than a 10x growth off millions.

## Adding a query

Add one `emit(...)` line in `examples/graph_audit/main.rs`, calling the real
query function with `audit_*` ids the seed creates. If the signature changes,
the bin stops compiling, which is the point. If the query needs an entity the
seed doesn't make yet, extend `seed.sh` (keep the `:Bench` label).

## What to look for (grounded in the Neo4j 5.26 docs)

Each anti-pattern, how it shows in `PROFILE` (`run.sh` keeps `--format verbose`
output if you profile a single query by hand), and the fix:

| Anti-pattern | In the plan | Fix |
|---|---|---|
| Row-multiplying `MATCH`/`OPTIONAL MATCH` upstream of per-row `COUNT{}`/`EXISTS{}` in an aggregating `WITH` (#935) | subquery operators with Rows ≈ N×, huge DB Hits | independent `COUNT{}` off a single row, no leading fan-out |
| Several independent `OPTIONAL MATCH` then `COUNT(DISTINCT)` | `Expand` cardinality = product of branches | one `CALL { }` subquery per count |
| Disconnected patterns | `CartesianProduct` (M×N rows) | connect patterns / split into subqueries |
| Property lookup not using an index | `AllNodesScan` / `NodeByLabelScan` instead of `NodeIndexSeek` | add index/constraint (see `db/graph/setup.rs`) |
| Unbounded variable-length expansion (`FOLLOWS*1..n`) | `VarLengthExpand(All)`, exploding Rows | cap depth, bound fan-out, or precompute |
| Eager materialization / large sort | `Eager`, `EagerAggregation`, `Sort`, `Top` (check Memory) | reduce cardinality before; index-backed order |

Why the #935 mechanism is real (doc-confirmed): in an aggregating `WITH`, **all
non-aggregating columns become grouping keys**
([Aggregating functions](https://neo4j.com/docs/cypher-manual/current/functions/aggregating/)),
and a `COUNT{}` subquery expression **executes per input row**
([COUNT subqueries](https://neo4j.com/docs/cypher-manual/current/subqueries/count/),
[KB: scope of aggregations](https://neo4j.com/developer/kb/using-subqueries-to-control-the-scope-of-aggregations/)).
A `COUNT{}` as a grouping-key column sitting on a row-multiplying `OPTIONAL
MATCH` is therefore evaluated once per row → O(rows × subquery cost).

## Limitations (read before trusting a 1.0x)

- **Profiles the checked-out branch.** On `main`, `user_counts` still shows the
  #935 bug; on a branch with the fix it drops. Re-run after merges.
- **Growth only reflects dimensions the seed scales** (posts, followers,
  following, received tags, hot-post engagement, active followers, resource
  taggers). A query flat at 1.0x may simply have an unscaled dimension
  (e.g. a user's own bookmark count is fixed here), not be inherently flat.
- **Read queries only.** `put`/`del` queries mutate, so profiling them needs a
  rollback wrapper (not yet implemented).
- **Two-point growth**, not a full curve. Two scales catch super-linear vs
  linear; for a precise exponent, seed several scales by hand.
