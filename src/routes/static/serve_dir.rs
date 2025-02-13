use once_cell::sync::OnceCell;
use tower_http::services::ServeDir;

use crate::Config;

static SERVE_DIR_INSTANCE: OnceCell<ServeDir> = OnceCell::new();

pub fn get_serve_dir() -> ServeDir {
    SERVE_DIR_INSTANCE
        .get_or_init(|| {
            let config = Config::from_env();
            ServeDir::new(config.file_path)
        })
        .to_owned()
}
