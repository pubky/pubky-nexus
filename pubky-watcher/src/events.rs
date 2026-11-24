//! The `/events/` line protocol: a `text/plain` body with one event per line
//! (`PUT <uri>` / `DEL <uri>`), closed by a `cursor: <value>` line.

use std::convert::Infallible;

use crate::client::EventMethod;
use crate::traits::{LineParseOutcome, ParseFromLine};

/// Prefix of the line closing a homeserver `/events/` response with its cursor.
pub const CURSOR_PREFIX: &str = "cursor: ";

/// An event line of a homeserver's `/events/` feed: `PUT <uri>` or `DEL <uri>`.
///
/// The event type [`Watcher::homeserver`](crate::Watcher::homeserver) hands to its handler.
/// An application that needs richer parsing defines its own type and implements
/// [`ParseFromLine`] for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomeserverEvent {
    /// `PUT` or `DEL`.
    pub method: EventMethod,
    /// The resource's `pubky://` URI, as the line carries it.
    pub uri: String,
}

/// Parsing never fails: an empty line is [`LineParseOutcome::Skipped`], and a line that is
/// not `PUT <uri>` or `DEL <uri>` is [`LineParseOutcome::Unrecognized`]. Only the first space
/// separates the method from the URI: homeservers write a stored file's name unencoded, and
/// a name may contain spaces.
impl ParseFromLine for HomeserverEvent {
    type Error = Infallible;

    fn parse_line(line: &str) -> Result<LineParseOutcome<Self>, Self::Error> {
        if line.trim().is_empty() {
            return Ok(LineParseOutcome::Skipped);
        }

        let Some((method, uri)) = line.split_once(' ') else {
            return Ok(LineParseOutcome::Unrecognized {
                reason: format!("expected `<METHOD> <uri>`, got: {line}"),
            });
        };

        let method = match method {
            "PUT" => EventMethod::Put,
            "DEL" => EventMethod::Del,
            other => {
                return Ok(LineParseOutcome::Unrecognized {
                    reason: format!("unknown event method: {other}"),
                });
            }
        };

        if uri.trim().is_empty() {
            return Ok(LineParseOutcome::Unrecognized {
                reason: format!("missing uri in event line: {line}"),
            });
        }

        Ok(LineParseOutcome::Parsed(Self {
            method,
            uri: uri.to_string(),
        }))
    }
}

/// A homeserver `/events/` response, split into its event lines and the cursor closing the batch.
///
/// The homeserver sends the cursor line last, but that is positional convention
/// rather than something we can rely on from an untrusted peer: the split is
/// order-independent, and if a batch carries several cursor lines the last one
/// wins, being the position the homeserver ends the batch at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventBatch<'a> {
    /// The event lines, in the order the homeserver sent them, for
    /// [`ParseFromLine::parse_line`](crate::ParseFromLine::parse_line).
    pub event_lines: Vec<&'a str>,
    /// Raw, still unparsed value of the batch's cursor line, if it carried one.
    pub cursor: Option<&'a str>,
}

impl<'a> EventBatch<'a> {
    /// Splits the lines of a `/events/` response.
    pub fn split(lines: &'a [String]) -> Self {
        let mut event_lines = Vec::with_capacity(lines.len());
        let mut cursor = None;

        for line in lines {
            match line.strip_prefix(CURSOR_PREFIX) {
                Some(value) => cursor = Some(value),
                None => event_lines.push(line.as_str()),
            }
        }

        Self {
            event_lines,
            cursor,
        }
    }

    /// Whether the batch carried anything other than its cursor line.
    ///
    /// Deliberately counts *every* non-cursor line, including ones that later
    /// parse as skipped or unrecognized: those still legitimately advance the
    /// cursor, so narrowing this to "lines that reached a handler" would let a
    /// non-advancing cursor through and re-open the replay loop.
    pub fn has_events(&self) -> bool {
        !self.event_lines.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::{EventBatch, HomeserverEvent};
    use crate::{EventMethod, LineParseOutcome, ParseFromLine};

    fn parsed(line: &str) -> HomeserverEvent {
        match HomeserverEvent::parse_line(line).unwrap() {
            LineParseOutcome::Parsed(event) => event,
            other => panic!("{line:?} did not parse: {other:?}"),
        }
    }

    fn unrecognized(line: &str) -> bool {
        matches!(
            HomeserverEvent::parse_line(line).unwrap(),
            LineParseOutcome::Unrecognized { .. }
        )
    }

    // --- HomeserverEvent::parse_line ---

    #[test]
    fn put_and_del_lines_parse() {
        assert_eq!(
            parsed("PUT pubky://a/pub/x"),
            HomeserverEvent {
                method: EventMethod::Put,
                uri: "pubky://a/pub/x".into()
            }
        );
        assert_eq!(parsed("DEL pubky://a/pub/x").method, EventMethod::Del);
    }

    /// Only the first space separates the method from the URI, so a file name with spaces
    /// stays in the URI.
    #[test]
    fn the_uri_runs_from_the_first_space_to_the_end() {
        assert_eq!(
            parsed("PUT pubky://a/pub/files/my holiday photo.jpg").uri,
            "pubky://a/pub/files/my holiday photo.jpg"
        );
    }

    #[test]
    fn empty_lines_are_skipped() {
        assert!(matches!(
            HomeserverEvent::parse_line("").unwrap(),
            LineParseOutcome::Skipped
        ));
    }

    #[test]
    fn other_lines_are_unrecognized() {
        assert!(unrecognized("PUT"));
        assert!(unrecognized("PUT "));
        assert!(unrecognized("PATCH pubky://a/pub/x"));
        assert!(unrecognized("put pubky://a/pub/x"));
    }

    /// A cursor line that reaches the parser is no event either.
    #[test]
    fn a_cursor_line_is_unrecognized() {
        assert!(unrecognized("cursor: 42"));
    }

    fn lines(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|l| l.to_string()).collect()
    }

    // --- EventBatch::split ---

    /// The usual shape: events followed by the cursor that closes the batch.
    #[test]
    fn splits_events_from_the_trailing_cursor() {
        let lines = lines(&["PUT pubky://a/pub/x", "DEL pubky://b/pub/y", "cursor: 42"]);
        let batch = EventBatch::split(&lines);

        assert_eq!(
            batch.event_lines,
            vec!["PUT pubky://a/pub/x", "DEL pubky://b/pub/y"]
        );
        assert_eq!(batch.cursor, Some("42"));
        assert!(batch.has_events());
    }

    /// An idle poll: a cursor and nothing else, which must not read as "had events".
    #[test]
    fn cursor_only_batch_has_no_events() {
        let lines = lines(&["cursor: 42"]);
        let batch = EventBatch::split(&lines);

        assert!(batch.event_lines.is_empty());
        assert_eq!(batch.cursor, Some("42"));
        assert!(!batch.has_events());
    }

    /// The cursor is found wherever it sits, not only in last position.
    #[test]
    fn cursor_is_found_out_of_position() {
        let lines = lines(&["cursor: 42", "PUT pubky://a/pub/x"]);
        let batch = EventBatch::split(&lines);

        assert_eq!(batch.event_lines, vec!["PUT pubky://a/pub/x"]);
        assert_eq!(batch.cursor, Some("42"));
    }

    /// Several cursor lines: the last one wins, being where the batch ends.
    #[test]
    fn last_cursor_line_wins() {
        let lines = lines(&["cursor: 41", "PUT pubky://a/pub/x", "cursor: 42"]);
        let batch = EventBatch::split(&lines);

        assert_eq!(batch.event_lines, vec!["PUT pubky://a/pub/x"]);
        assert_eq!(batch.cursor, Some("42"));
    }

    /// A batch with no cursor line at all, which the caller rejects as stalled
    /// when it carries events: there is nothing to move the checkpoint with.
    #[test]
    fn batch_without_cursor_line() {
        let lines = lines(&["PUT pubky://a/pub/x"]);
        let batch = EventBatch::split(&lines);

        assert_eq!(batch.event_lines, vec!["PUT pubky://a/pub/x"]);
        assert_eq!(batch.cursor, None);
        assert!(batch.has_events());
    }

    /// Lines that will later parse as skipped or unrecognized still count as
    /// events: they legitimately advance the cursor, so a batch made only of
    /// them must not be treated as an idle poll.
    #[test]
    fn unrecognized_lines_count_as_events() {
        let lines = lines(&["garbage", "cursor: 42"]);
        let batch = EventBatch::split(&lines);

        assert_eq!(batch.event_lines, vec!["garbage"]);
        assert!(batch.has_events());
    }
}
