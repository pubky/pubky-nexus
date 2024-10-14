#[macro_export]
macro_rules! register_routes {
    ($router:expr, $($path:expr => $handler:expr),* $(,)?) => {
        $router
            $(.route($path, axum::routing::get($handler)))*
    };
}

/// Transforms a "swagger-like" endpoint to "axum-like" manipulating &'static str
/// Example `"/v1/user/{user_id}"` -> `"/v1/user/:user_id"`
#[macro_export]
macro_rules! to_axum {
    ($route:expr) => {
        str_replace!(str_replace!($route, "{", ":"), "}", "")
    };
}
