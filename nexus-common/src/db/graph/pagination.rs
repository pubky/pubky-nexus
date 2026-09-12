use crate::db::graph::GraphError;
use std::fmt::Debug;
use std::future::Future;

/// Drives a keyset-paginated scan over graph rows, paging on a single sort column.
///
/// `step(cursor)` fetches one batch starting after `cursor` and returns `(count, last_cursor)`.
/// The loop stops when `count < batch_size` or the batch is empty; the scan starts before the
/// first row with an empty cursor.
pub async fn keyset_scan<F, Fut, E>(batch_size: usize, context: &str, step: F) -> Result<(), E>
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = Result<(usize, Option<String>), E>>,
    E: From<GraphError>,
{
    scan(batch_size, context, step).await
}

/// [`keyset_scan`] for a query that pages on a row value, e.g. `ORDER BY tagger.id, t.id`.
///
/// The cursor carries the sort columns themselves, in `ORDER BY` order, so it needs no
/// encoding and compares exactly as the database ordered the rows. A single-column query can
/// use a one-element cursor to share a signature with composite ones.
pub async fn keyset_scan_composite<F, Fut, E>(
    batch_size: usize,
    context: &str,
    step: F,
) -> Result<(), E>
where
    F: FnMut(Vec<String>) -> Fut,
    Fut: Future<Output = Result<(usize, Option<Vec<String>>), E>>,
    E: From<GraphError>,
{
    scan(batch_size, context, step).await
}

/// Shared driver. A full batch must yield a cursor strictly greater than the one it was given,
/// ordered the same way as the query's `ORDER BY`; anything else means the scan cannot advance
/// and is reported rather than silently dropping the rest of the scan.
async fn scan<C, F, Fut, E>(batch_size: usize, context: &str, mut step: F) -> Result<(), E>
where
    C: Ord + Clone + Default + Debug,
    F: FnMut(C) -> Fut,
    Fut: Future<Output = Result<(usize, Option<C>), E>>,
    E: From<GraphError>,
{
    let mut cursor = C::default();
    loop {
        let (count, last_cursor) = step(cursor.clone()).await?;
        if count == 0 || count < batch_size {
            break;
        }
        let Some(next) = last_cursor else {
            return Err(stalled(
                context,
                format!("full batch of {count} rows produced no cursor"),
            ));
        };
        if next <= cursor {
            return Err(stalled(
                context,
                format!("cursor did not advance past {cursor:?}, got {next:?}"),
            ));
        }
        cursor = next;
    }
    Ok(())
}

/// A stall is a data anomaly, not a transient failure: callers may drop the event, so warn here.
fn stalled<E: From<GraphError>>(context: &str, detail: String) -> E {
    tracing::warn!(context, detail, "Keyset scan stalled");
    GraphError::KeysetScanStalled {
        context: context.to_string(),
        detail,
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::{keyset_scan, keyset_scan_composite, GraphError};
    use std::cell::RefCell;

    /// Stand-in for whatever error type the caller's `step` returns.
    #[derive(Debug, PartialEq)]
    enum TestError {
        Step(&'static str),
        /// The rendered `GraphError`; the type itself is not `PartialEq`.
        Graph(String),
    }

    impl From<GraphError> for TestError {
        fn from(e: GraphError) -> Self {
            TestError::Graph(e.to_string())
        }
    }

    /// One canned page returned by the fake `step`.
    type Page = Result<(usize, Option<String>), TestError>;
    /// One canned page for the row-value wrapper.
    type CompositePage = Result<(usize, Option<Vec<String>>), TestError>;

    struct Case {
        name: &'static str,
        batch_size: usize,
        pages: Vec<Page>,
        /// The cursors `keyset_scan` is expected to pass to `step`, in order.
        expected_cursors: Vec<&'static str>,
    }

    /// Serves `pages` in order, recording every cursor it was called with.
    async fn scan_over(
        batch_size: usize,
        pages: Vec<Page>,
    ) -> (Result<(), TestError>, Vec<String>) {
        let cursors = RefCell::new(Vec::new());
        let pages = RefCell::new(pages.into_iter());

        let result = keyset_scan(batch_size, "test", |cursor| {
            cursors.borrow_mut().push(cursor);
            let page = pages
                .borrow_mut()
                .next()
                .expect("step called more times than there are pages");
            async move { page }
        })
        .await;

        (result, cursors.into_inner())
    }

    #[tokio::test]
    async fn table_driven_scans() {
        let cases = vec![
            Case {
                name: "empty first page stops immediately",
                batch_size: 3,
                pages: vec![Ok((0, None))],
                expected_cursors: vec![""],
            },
            Case {
                name: "short first page stops after one call",
                batch_size: 3,
                pages: vec![Ok((2, Some("b".into())))],
                expected_cursors: vec![""],
            },
            Case {
                name: "full page is followed by another call from the last cursor",
                batch_size: 2,
                pages: vec![Ok((2, Some("b".into()))), Ok((1, Some("c".into())))],
                expected_cursors: vec!["", "b"],
            },
            Case {
                // A final page of exactly batch_size costs one extra empty round-trip.
                name: "exact multiple of batch_size needs a trailing empty page",
                batch_size: 2,
                pages: vec![Ok((2, Some("b".into()))), Ok((0, None))],
                expected_cursors: vec!["", "b"],
            },
            Case {
                name: "cursor advances across several full pages",
                batch_size: 2,
                pages: vec![
                    Ok((2, Some("b".into()))),
                    Ok((2, Some("d".into()))),
                    Ok((1, Some("e".into()))),
                ],
                expected_cursors: vec!["", "b", "d"],
            },
        ];

        for case in cases {
            let (result, cursors) = scan_over(case.batch_size, case.pages).await;
            let name = case.name;
            assert_eq!(result, Ok(()), "{name}: expected the scan to succeed");
            assert_eq!(
                cursors, case.expected_cursors,
                "{name}: cursor sequence differs"
            );
        }
    }

    #[tokio::test]
    async fn full_page_without_cursor_is_an_error() {
        let (result, cursors) = scan_over(2, vec![Ok((2, None))]).await;

        assert_eq!(
            result,
            Err(TestError::Graph(
                "test: keyset scan cannot advance (full batch of 2 rows produced no cursor)"
                    .to_string()
            ))
        );
        assert_eq!(cursors, vec![""], "the scan must not ask for another page");
    }

    #[tokio::test]
    async fn non_advancing_cursor_is_an_error() {
        // Repeating or rewinding the cursor would refetch the same page forever.
        let cases = [Some("b".to_string()), Some("a".to_string())];

        for next in cases {
            let (result, cursors) =
                scan_over(2, vec![Ok((2, Some("b".into()))), Ok((2, next.clone()))]).await;

            let got = next.clone().expect("case supplies a cursor");
            assert_eq!(
                result,
                Err(TestError::Graph(format!(
                    "test: keyset scan cannot advance (cursor did not advance past \"b\", got {got:?})"
                ))),
                "cursor {next:?} must not be accepted after \"b\""
            );
            assert_eq!(cursors, vec!["", "b"]);
        }
    }

    #[tokio::test]
    async fn short_page_without_cursor_is_fine() {
        // Only a *full* page needs a cursor; a short page already ends the scan.
        let (result, _) = scan_over(2, vec![Ok((1, None))]).await;

        assert_eq!(result, Ok(()));
    }

    /// The composite wrapper must order by column, not by any flattened encoding.
    #[tokio::test]
    async fn composite_cursor_advances_by_row_value() {
        let pages: Vec<CompositePage> = vec![
            Ok((1, Some(vec!["a".into(), "bc".into()]))),
            // Both rows flatten to "abc", so a joined cursor would read as stalled here;
            // a row value compares column by column and sees "a" < "ab".
            Ok((1, Some(vec!["ab".into(), "c".into()]))),
            Ok((0, None)),
        ];
        let cursors = RefCell::new(Vec::new());
        let pages = RefCell::new(pages.into_iter());

        let result = keyset_scan_composite(1, "test", |cursor| {
            cursors.borrow_mut().push(cursor);
            let page = pages.borrow_mut().next().expect("page available");
            async move { page }
        })
        .await;

        assert_eq!(result, Ok(()));
        assert_eq!(
            cursors.into_inner(),
            vec![
                Vec::<String>::new(),
                vec!["a".to_string(), "bc".to_string()],
                vec!["ab".to_string(), "c".to_string()],
            ],
            "each page must resume from the previous row value"
        );
    }

    #[tokio::test]
    async fn step_error_propagates() {
        let (result, cursors) = scan_over(
            2,
            vec![Ok((2, Some("b".into()))), Err(TestError::Step("boom"))],
        )
        .await;

        assert_eq!(result, Err(TestError::Step("boom")));
        assert_eq!(cursors, vec!["", "b"]);
    }
}
