use crate::{
    models::file::{details::FileKey, FileDetails},
    Config,
};
use axum::{
    extract::Request,
    middleware::{self, Next},
    response::Response,
    routing::get_service,
    Router,
};
use reqwest::StatusCode;
use tower_http::services::ServeDir;

async fn static_files_middleware(request: Request, next: Next) -> Result<Response, StatusCode> {
    let path = String::from(request.uri().path());

    let mut response = next.run(request).await;

    if response.status() != StatusCode::OK {
        return Ok(response);
    }

    let [_, _, owner_id, file_id]: [&str] = path.split("/").collect::<Vec<&str>>()[..] else {
        return Ok(response);
    };

    let file_key = FileKey {
        owner_id: owner_id.to_string(),
        file_id: file_id.to_string(),
    };

    let file = FileDetails::get_file(&file_key).await;

    match file {
        Ok(Some(value)) => {
            response.headers_mut().insert(
                "Content-Length",
                value.size.to_string().as_str().parse().unwrap(),
            );
            response
                .headers_mut()
                .insert("Content-Type", value.content_type.parse().unwrap());
            Ok(response)
        }
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub fn routes() -> Router {
    let config = Config::from_env();
    Router::new()
        .nest_service("/static", get_service(ServeDir::new(config.static_path)))
        .route_layer(middleware::from_fn(static_files_middleware))
}
