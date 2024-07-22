use axum::Router;

pub mod macros;
pub mod r#static;
pub mod v0;

pub fn routes() -> Router {
    let routes_v0 = v0::routes();
    let route_static = r#static::routes();

    routes_v0.merge(route_static)
}
