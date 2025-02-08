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
