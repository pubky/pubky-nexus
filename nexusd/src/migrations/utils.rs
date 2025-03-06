pub fn to_snake_case(input: &str) -> String {
    let mut result = String::new();
    let mut prev_was_upper = false;

    for (i, c) in input.chars().enumerate() {
        if c.is_uppercase() {
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

pub fn generate_template(name: &str) -> String {
    format!(
        "use async_trait::async_trait;

use crate::migrations::manager::Migration;
use nexus_common::types::DynError;

pub struct {name};

#[async_trait]
impl Migration for {name} {{
    fn id(&self) -> &'static str {{
        \"{name}\"
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
",
        name = name
    )
}

#[cfg(test)]
mod tests {
    #[tokio_shared_rt::test(shared)]
    async fn test_to_snake_case() {
        assert_eq!(super::to_snake_case("CamelCase"), "camel_case");
        assert_eq!(super::to_snake_case("PascalCase"), "pascal_case");
        assert_eq!(super::to_snake_case("snake_case"), "snake_case");
        assert_eq!(super::to_snake_case("kebab-case"), "kebab-case");
        assert_eq!(super::to_snake_case("UPPERCASE"), "uppercase");
        assert_eq!(super::to_snake_case("lowercase"), "lowercase");
        assert_eq!(super::to_snake_case("12345"), "12345");
        assert_eq!(super::to_snake_case("snake_case_123"), "snake_case_123");
        assert_eq!(super::to_snake_case("UserNewField"), "user_new_field");
    }
}
