// pub mod posts;
pub mod profile;
// pub mod search;
// pub mod trending;

pub struct Tag {
    id: String, // TODO: create Crockfordbase32 Struct and validator
    name: String,
    indexed_at: i64
}