# pubky-watcher

Read events from [Pubky](https://pubky.org) homeservers, for applications that index them:
search engines, moderation tools, archives, application-specific indexes.

A homeserver emits one line per change, `PUT pubky://<user>/<path>` or
`DEL pubky://<user>/<path>`, plus a cursor. Neither kind carries a body: for a `PUT`, the
application fetches the resource itself. This crate reads those events and hands each one to
your handler. What an event means, where it is stored, where cursors are kept, and when a
failure is retried stay with your application.

Status: `0.1.0`, not published. The boundary and the names are still under discussion.

## Quick start: one homeserver

```rust,ignore
use pubky::EventCursor;
use pubky_watcher::{EventHandler, HomeserverEvent, Watcher, WatcherClient};

struct Print;

/// The handler's own error type: a failed run gives it back as `WatcherError::Handler`.
#[derive(Debug, thiserror::Error)]
#[error("could not print the event")]
struct PrintError;

#[async_trait::async_trait]
impl EventHandler<HomeserverEvent, PrintError> for Print {
    async fn handle(&self, event: &HomeserverEvent) -> Result<(), PrintError> {
        println!("{} {}", event.method, event.uri);
        Ok(())
    }
}

let watcher = Watcher::homeserver(WatcherClient::mainnet()?, homeserver)
    .handler(Print)
    .build(shutdown_rx)?;

let outcome = watcher.run(EventCursor::new(stored_cursor)).await?;
// Persist `outcome.cursor`, and start the next run from it.
```

`Watcher::key_stream(client, homeserver, users)` reads a known set of users instead, one
`/events-stream` each, with one cursor per user and an optional path prefix (`/pub/` by
default).

What a run guarantees:

- **The cursor it returns is safe to persist.** Every event before it was handled.
- **Shutdown loses nothing.** A run stopped by its shutdown signal returns `Ok` with
  `completed = false` and a cursor that is still safe to persist.
- **An idle homeserver is cheap.** It answers no events, and the cursor stays where it was.
- **A hostile or broken homeserver cannot move you backwards.** A `/events/` batch must be a
  success, within the size cap, and carry a numeric cursor that moves forward when it has
  events. A key stream must return no more events than requested, only the requested user's,
  with cursors that only move forward. A batch that breaks this fails the run before any of
  its events reaches your handler.
- **Misconfiguration fails at build time:** an events limit of zero, an empty or duplicated
  user list, or a path that does not start with `/`.

`build` fails with a `ConfigError`. A run fails with a `WatcherError<E>`, where `E` is your
handler's error: `Homeserver` (an `HsError`), `Protocol` (a `ProtocolError`: which rule the
homeserver broke, with the cursors and users involved), `Handler(E)`, or `Panicked`.

## The client

`WatcherClient` makes every homeserver call. Build it with `WatcherClient::mainnet()`,
`WatcherClient::testnet(host)`, or over an SDK client you already have with `from_pubky` or
`from_shared`. `sdk()` returns that SDK client for calls the watcher does not cover.

Its capabilities are split into narrow traits, so that your tests fake only the one they
exercise:

| Trait | What it does |
|---|---|
| `HomeserverEventSource` | a homeserver's `/events/` feed |
| `KeyEventSource` | a user's `/events-stream` (a `429` is retried inside the client) |
| `ResourceReader` | a `PUT` event's resource or blob, under a size cap |
| `HomeserverResolver` | the signed PKDNS record a user publishes; `published_homeserver` reads the homeserver it names |

The resolver returns the raw signed record, so that you can keep it, check its timestamp,
and refuse an older answer that is replayed or lagging.

A failed call is an `HsError`: the operation (poll events, user events, get resource, get
blob) and a kind (not found, rate limited, server error, rejected, timeout, transport, too
large, SDK, malformed), so that you can choose your own retry policy.

## Advanced: processors and runners

The builders are built on lower-level traits, which an application with its own cursor store,
retry queue or backoff implements directly:

- `ParseFromLine` parses an event line into your event type.
- `TEventProcessor<E, Err>` is one run over one homeserver. Each run is waited for on its own
  task, logged as slow after a threshold, and reported as failed if it panics. Its
  `handle_error` fails fast by default.
- `TEventProcessorRunner<E, Err>` runs one processor per homeserver, one after the other, and
  reports run statistics (succeeded, failed, panicked, budget spent, skipped). A hook decides
  which homeservers to skip, e.g. those in your backoff.
- Retry is opt-in: implement `RetryableError` for your error type and `EventRetryScheduler`
  for your queue, and call `dispatch_retryable_error` from `handle_error`.

## Examples

See [`examples/`](examples/README.md): one per builder and one for the low-level traits.

## Dependencies

Homeservers and users are `pubky::PublicKey`s. The crate depends on the Pubky SDK, `reqwest`,
`tokio`, `futures` and `tracing`, and brings no database, metrics or application config
with it.
