pub fn to_snake_case(input: &str) -> String {
    let mut result = String::new();
    let mut prev_was_upper = false;

    for (i, c) in input.chars().enumerate() {
        if c == '-' {
            // Convert hyphens to underscores
            result.push('_');
            prev_was_upper = false;
        } else if c.is_uppercase() {
            if i > 0 && !prev_was_upper {
                result.push('_');
            }
            result.push(c.to_ascii_lowercase());
            prev_was_upper = true;
        } else {
            result.push(c);
            prev_was_upper = false;
        }
    }

    result
}

pub fn to_pascal_case(input: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = true;

    for c in input.chars() {
        if c == '_' || c == '-' {
            // Handle underscores and hyphens
            capitalize_next = true;
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }

    result
}

pub fn generate_template(name: &str) -> String {
    let struc_name = to_pascal_case(name);
    format!(
        "use async_trait::async_trait;

use crate::migrations::manager::Migration;
use nexus_common::types::DynError;

pub struct {struc_name};

#[async_trait]
impl Migration for {struc_name} {{
    fn id(&self) -> &'static str {{
        \"{struc_name}\"
    }}  
        
    fn is_multi_staged(&self) -> bool {{
        true
    }}  
        
    async fn dual_write(data: Box<dyn std::any::Any + Send + 'static>) -> Result<(), DynError> {{
        // Implement your dual write logic here. Downcast data to your struct type.
        Ok(())
    }}  
    
    async fn backfill(&self) -> Result<(), DynError> {{
        // Your backfill logic here
        Ok(())
    }}  
                
    async fn cutover(&self) -> Result<(), DynError> {{
        // Your cutover logic here
        Ok(())  
    }}  
                    
    async fn cleanup(&self) -> Result<(), DynError> {{
        // Your cleanup logic here
        Ok(())
    }}
}}
"
    )
}

#[cfg(test)]
mod tests {
    #[tokio_shared_rt::test(shared)]
    async fn test_to_snake_case() {
        assert_eq!(super::to_snake_case("CamelCase"), "camel_case");
        assert_eq!(super::to_snake_case("PascalCase"), "pascal_case");
        assert_eq!(super::to_snake_case("snake_case"), "snake_case");
        assert_eq!(super::to_snake_case("kebab-case"), "kebab_case");
        assert_eq!(super::to_snake_case("UPPERCASE"), "uppercase");
        assert_eq!(super::to_snake_case("lowercase"), "lowercase");
        assert_eq!(super::to_snake_case("12345"), "12345");
        assert_eq!(super::to_snake_case("snake_case_123"), "snake_case_123");
        assert_eq!(super::to_snake_case("UserNewField"), "user_new_field");
    }

    #[tokio_shared_rt::test(shared)]
    async fn test_to_pascal_case() {
        assert_eq!(super::to_pascal_case("camel_case"), "CamelCase");
        assert_eq!(super::to_pascal_case("pascal_case"), "PascalCase");
        assert_eq!(super::to_pascal_case("snake_case"), "SnakeCase");
        assert_eq!(super::to_pascal_case("kebab-case"), "KebabCase");
        assert_eq!(super::to_pascal_case("uppercase"), "Uppercase");
        assert_eq!(super::to_pascal_case("lowercase"), "Lowercase");
        assert_eq!(super::to_pascal_case("12345"), "12345");
        assert_eq!(super::to_pascal_case("snake_case_123"), "SnakeCase123");
        assert_eq!(super::to_pascal_case("user_new_field"), "UserNewField");
    }
}
