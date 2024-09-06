use crate::{
    models::{file::FileDetails, traits::Collection},
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

    let files = FileDetails::get_by_ids(vec![vec![owner_id, file_id].as_slice()].as_slice()).await;

    match files {
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
        Ok(value) => {
            let file = &value[0];
            match file {
                Some(value) => {
                    response.headers_mut().insert(
                        "Content-Length",
                        value.size.to_string().as_str().parse().unwrap(),
                    );
                    response
                        .headers_mut()
                        .insert("Content-Type", value.content_type.parse().unwrap());
                    Ok(response)
                }
                None => Err(StatusCode::NOT_FOUND),
            }
        }
    }
}

pub fn routes() -> Router {
    let config = Config::from_env();

    let general =
        Router::new().nest_service("/static/", get_service(ServeDir::new(config.static_path)));

    let files = Router::new()
        .nest_service(
            "/static/files/",
            get_service(ServeDir::new(config.file_path)),
        )
        .route_layer(middleware::from_fn(static_files_middleware));

    general.merge(files)
}
