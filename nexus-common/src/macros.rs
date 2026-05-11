/// Runs multiple futures concurrently via [`tokio::join!`] inside an
/// instrumented async block, removing the need to manually wrap the call in
/// `async { … }.instrument(span).await`.
///
/// # Syntax
///
/// ```ignore
/// traced_join!(span_expression; future1, future2, …)
/// ```
///
/// The span is separated from the futures by a **semicolon**.  The returned
/// value is the same tuple that `tokio::join!` would produce, so callers can
/// destructure or index into it as usual.
///
/// # Example
///
/// ```ignore
/// let results = traced_join!(
///     tracing::info_span!("index.write");
///     followers.put_to_index(&followee_id),
///     following.put_to_index(&follower_id),
/// );
/// results.0?;
/// results.1?;
/// ```
#[macro_export]
macro_rules! traced_join {
    ($span:expr; $($fut:expr),+ $(,)?) => {{
        use ::tracing::Instrument as _;
        async { ::tokio::join!($($fut),+) }.instrument($span).await
    }};
}
