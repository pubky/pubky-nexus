/// Defines multiple GET routes on an Axum `Router` by allowing
/// multiple path-handler pairs to be specified inline
///
/// # Expansion
///
/// ```rust
/// let router = router
///     .route("/route1", get(handler1))
///     .route("/route2", get(handler2));
/// ```
#[macro_export]
macro_rules! register_routes {
    ($router:expr, $($path:expr => $handler:expr),* $(,)?) => {
        $router
            $(.route($path, axum::routing::get($handler)))*
    };
}

/// This macro allows defining multiple GET routes for an Axum `Router`, while also
/// associating it with a shared state object. It ensures that each route has access
/// to the provided state
///
/// # Expansion
///
/// ```rust
/// let router = router.with_state(app_state.clone())
///     .route("/route1", get(handler1))
///     .route("/route2", get(handler2));
/// ```
#[macro_export]
macro_rules! register_routes_with_state {
    ($router:expr, $state:expr, $($path:expr => $handler:expr),* $(,)?) => {
        {
            let router = $router.with_state($state.clone());
            $(let router = router.route($path, axum::routing::get($handler));)*
            router
        }
    };
}
