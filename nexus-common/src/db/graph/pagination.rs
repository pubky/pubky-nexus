use std::future::Future;

/// Drives a keyset-paginated scan over graph rows.
///
/// `step(cursor)` fetches one batch starting after `cursor` and returns
/// `(count, last_cursor)`.  The loop stops when `count < batch_size` or
/// the batch is empty.  A full batch with no cursor would repeat forever,
/// so the scan logs and stops instead.
pub async fn keyset_scan<F, Fut, E>(batch_size: usize, context: &str, mut step: F) -> Result<(), E>
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = Result<(usize, Option<String>), E>>,
{
    let mut cursor = String::new();
    loop {
        let (count, last_cursor) = step(cursor).await?;
        if count == 0 || count < batch_size {
            break;
        }
        match last_cursor {
            Some(next) => cursor = next,
            None => {
                tracing::error!("{context}: batch of {count} rows produced no usable cursor");
                break;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::keyset_scan;
    use std::cell::RefCell;

    /// Stand-in for whatever error type the caller's `step` returns.
    #[derive(Debug, PartialEq)]
    struct TestError(&'static str);

    /// One canned page returned by the fake `step`.
    type Page = Result<(usize, Option<String>), TestError>;

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
    async fn full_page_without_cursor_stops_instead_of_looping() {
        let (result, cursors) = scan_over(2, vec![Ok((2, None))]).await;

        assert_eq!(result, Ok(()));
        assert_eq!(cursors, vec![""], "the scan must not ask for another page");
    }

    #[tokio::test]
    async fn short_page_without_cursor_is_fine() {
        // Only a *full* page needs a cursor; a short page already ends the scan.
        let (result, _) = scan_over(2, vec![Ok((1, None))]).await;

        assert_eq!(result, Ok(()));
    }

    #[tokio::test]
    async fn step_error_propagates() {
        let (result, cursors) =
            scan_over(2, vec![Ok((2, Some("b".into()))), Err(TestError("boom"))]).await;

        assert_eq!(result, Err(TestError("boom")));
        assert_eq!(cursors, vec!["", "b"]);
    }
}
