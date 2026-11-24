# pubky-watcher examples

Each example builds a `WatcherClient` for mainnet and reads the public staging homeserver:

```text
ufibwbmed6jeq9k4p583go95wofakh9fwpp4k734trq79pd9u1uy
```

## `poll_homeserver_builder`

The quick path: `Watcher::homeserver` with an `EventHandler`. It polls the homeserver's
`/events/` feed five times, feeding each returned cursor into the next run, and reads each
`PUT`'s resource through `ResourceReader`.

```bash
cargo run -p pubky-watcher --example poll_homeserver_builder
```

## `poll_key_stream_builder`

Per user: `Watcher::key_stream` reads three users' `/events-stream`s on the homeserver and
returns one cursor per user.

```bash
cargo run -p pubky-watcher --example poll_key_stream_builder
```

## `poll_homeserver`

The low-level path: a `TEventProcessor` per poll, run by a `TEventProcessorRunner`, the way
Nexus wires the crate.

```bash
cargo run -p pubky-watcher --example poll_homeserver
```
