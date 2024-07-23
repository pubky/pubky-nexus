pub mod info;
pub mod post;
pub mod profile;

/// Trait with the default implementation to create
/// indexes prefixes using the Struct names as schema
pub trait Prefix {
    fn prefix() -> String {
        let type_name = std::any::type_name::<Self>();
        let struct_name = type_name.split("::").last().unwrap_or_default();
        format!("{}!", struct_name)
    }
}

// Blanket implementation for all types that meet the constraints
impl<T> Prefix for T {}
