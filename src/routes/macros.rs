#[macro_export]
macro_rules! register_routes {
    ($router:expr, $($path:expr => $handler:expr),* $(,)?) => {
        $router
            $(.route($path, axum::routing::get($handler)))*
    };
}
