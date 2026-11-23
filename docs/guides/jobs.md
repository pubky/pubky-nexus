# Scheduled jobs

Nexusd can run background jobs on a cron schedule. Jobs are configured in `config.toml` under `[jobs.<name>]` sections:

```toml
[jobs.hot-tags-cache-today]
cron = "0 0 3 * * *"  # every day at 03:00 (UTC)
```

The name after `jobs.` must be one that `nexusd jobs list` prints. `nexusd` refuses to start on a section that names no job.

The `cron` expression is SECONDS-FIRST, not the standard 5-field crontab — always include the seconds field: `sec min hour day-of-month month day-of-week [year]`. The trailing year field is optional. The same format is used for every schedule, from coarse to per-second granularity:

```toml
[jobs.influencers-cache-today]
cron = "0 0 * * * *"  # every hour, at minute 0 and second 0
```

## Available Commands

- **`nexusd jobs list`** — prints the names of all available jobs
- **`nexusd jobs run <name>`** — runs a single job immediately (on demand), bypassing the schedule

## Notes

- A job with no `[jobs.<name>]` section or no `cron` key is unscheduled (won't run automatically) but can still be triggered on demand. The exceptions are `influencers-cache-all-time` and `most-followed-cache`, which run on their own schedules while their sections are absent.
- `influencers-cache-all-time` is the only writer of the AllTime influencers ranking, the default `source=influencers` stream, so it runs once a day (02:04 UTC) with no configuration. A `[jobs.influencers-cache-all-time]` section with a `cron` changes the schedule. A section without one turns the job off, and then the ranking stops changing.
- `most-followed-cache` is the only writer of the most-followed ranking, the `source=most_followed` stream, so it runs once a day (04:06 UTC) with no configuration. A `[jobs.most-followed-cache]` section with a `cron` changes the schedule. A section without one turns the job off, and then the ranking stops changing.
- Scheduled jobs run only under `nexusd run`, the Docker image's default command, which also starts its own API and watcher. A deployment that runs `nexusd api` and `nexusd watcher` separately should either replace them with one `nexusd run`, or keep them and call `nexusd jobs run <name>` from an external cron. An on-demand run takes the same run lock as the scheduler.
- The `nexusd run` daemon validates all `[jobs.*]` sections at startup — a typo'd section name fails fast rather than being silently ignored.
- Running a job on demand with `nexusd jobs run` also validates the config, so a typo'd `[jobs.<name>]` section is caught regardless of how you invoke it.
