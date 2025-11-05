pub mod bootstrap;
pub mod event;
pub mod file;
pub mod follow;
pub mod homeserver;
pub mod notification;
pub mod post;
pub mod tag;
pub mod traits;
pub mod user;

/// Create tuples with a 0.0 score for each element, forcing the sorted set to support lexicographical search
fn create_zero_score_tuples(strings: &[String]) -> Vec<(f64, &str)> {
    strings.iter().map(|s| (0.0, s.as_str())).collect()
}
