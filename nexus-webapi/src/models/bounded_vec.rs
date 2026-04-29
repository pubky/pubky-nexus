/// Macro to create a bounded vector type with validated serialization and deserialization.
///
/// Supports two serialization formats:
/// - `comma_separated_string`: `"id1,id2,id3"` (for query parameters)
/// - `json_array`: `["id1", "id2", "id3"]` (for request bodies)
///
/// # Example
/// ```ignore
/// // Comma-separated string (default)
/// define_bounded_vec!(
///     name: PostIds,
///     element_type: PostId,
///     min: 0,
///     max: 100,
///     serialize_as: comma_separated_string,
/// );
///
/// // JSON array
/// define_bounded_vec!(
///     name: UserIds,
///     element_type: PubkyId,
///     min: 1,
///     max: 100,
///     serialize_as: json_array,
/// );
/// ```
#[macro_export]
macro_rules! define_bounded_vec {
    // === comma_separated_string variant ===
    (
        name: $Name:ident,
        element_type: $Element:ty,
        min: $min:expr,
        max: $max:expr,
        serialize_as: comma_separated_string,
    ) => {
        #[derive(::std::fmt::Debug, utoipa::ToSchema)]
        pub struct $Name(pub Vec<$Element>);

        impl $Name {
            pub fn validate(items: &[$Element]) -> Result<(), $crate::Error> {
                if $min > 0 && items.is_empty() {
                    return Err($crate::Error::invalid_input(&format!(
                        "At least {} item(s) required",
                        $min
                    )));
                }
                if items.len() > $max {
                    return Err($crate::Error::invalid_input(&format!(
                        "Maximum {} items allowed",
                        $max
                    )));
                }
                Ok(())
            }
        }

        impl ::std::ops::Deref for $Name {
            type Target = Vec<$Element>;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl ::std::fmt::Display for $Name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                for (i, item) in self.0.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    ::std::fmt::Display::fmt(item, f)?;
                }
                Ok(())
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $Name {
            fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
            where
                D: ::serde::de::Deserializer<'de>,
            {
                let s = ::std::string::String::deserialize(deserializer)?;
                let items: Vec<$Element> = if s.is_empty() {
                    Vec::new()
                } else {
                    s.split(',')
                        .map(|t| t.trim().to_string())
                        .filter(|t| !t.is_empty())
                        .map(|t| <$Element>::try_from(t).map_err(::serde::de::Error::custom))
                        .collect::<::std::result::Result<Vec<_>, _>>()?
                };
                <$Name>::validate(&items).map_err(::serde::de::Error::custom)?;
                Ok($Name(items))
            }
        }

        impl ::serde::Serialize for $Name {
            fn serialize<S>(&self, serializer: S) -> ::std::result::Result<S::Ok, S::Error>
            where
                S: ::serde::Serializer,
            {
                let s: ::std::string::String = self
                    .0
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                s.serialize(serializer)
            }
        }
    };

    // === json_array variant ===
    (
        name: $Name:ident,
        element_type: $Element:ty,
        min: $min:expr,
        max: $max:expr,
        serialize_as: json_array,
    ) => {
        #[derive(::std::fmt::Debug, utoipa::ToSchema)]
        pub struct $Name(pub Vec<$Element>);

        impl $Name {
            pub fn validate(items: &[$Element]) -> Result<(), $crate::Error> {
                if $min > 0 && items.is_empty() {
                    return Err($crate::Error::invalid_input(&format!(
                        "At least {} item(s) required",
                        $min
                    )));
                }
                if items.len() > $max {
                    return Err($crate::Error::invalid_input(&format!(
                        "Maximum {} items allowed",
                        $max
                    )));
                }
                Ok(())
            }
        }

        impl ::std::ops::Deref for $Name {
            type Target = Vec<$Element>;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl ::std::fmt::Display for $Name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                for (i, item) in self.0.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    ::std::fmt::Display::fmt(item, f)?;
                }
                Ok(())
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $Name {
            fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
            where
                D: ::serde::de::Deserializer<'de>,
            {
                // Parse raw JSON value first to check array length BEFORE element validation
                let raw = ::serde_json::Value::deserialize(deserializer)?;
                let arr = raw
                    .as_array()
                    .ok_or_else(|| ::serde::de::Error::custom("Expected an array"))?;

                // Fail fast: check length bounds before deserializing any element
                if arr.is_empty() && $min > 0 {
                    return Err(::serde::de::Error::custom(&format!(
                        "At least {} item(s) required",
                        $min
                    )));
                }
                if arr.len() > $max {
                    return Err(::serde::de::Error::custom(&format!(
                        "Maximum {} items allowed",
                        $max
                    )));
                }

                // Now safely deserialize elements (array is guaranteed within bounds)
                let items: Vec<$Element> =
                    ::serde_json::from_value(::serde_json::Value::Array(arr.clone()))
                        .map_err(::serde::de::Error::custom)?;

                Ok($Name(items))
            }
        }

        impl ::serde::Serialize for $Name {
            fn serialize<S>(&self, serializer: S) -> ::std::result::Result<S::Ok, S::Error>
            where
                S: ::serde::Serializer,
            {
                use serde::ser::SerializeSeq;
                let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
                for item in &self.0 {
                    seq.serialize_element(&item.to_string())?;
                }
                seq.end()
            }
        }
    };
}

#[cfg(test)]
mod tests {
    // Test comma_separated_string variant
    define_bounded_vec!(
        name: TestCommaIds,
        element_type: String,
        min: 0,
        max: 5,
        serialize_as: comma_separated_string,
    );

    // Test json_array variant
    define_bounded_vec!(
        name: TestJsonIds,
        element_type: String,
        min: 0,
        max: 5,
        serialize_as: json_array,
    );

    // === comma_separated_string tests ===

    #[test]
    fn comma_deserialize_valid() {
        let json = r#""a, b, c""#;
        let ids: TestCommaIds = serde_json::from_str(json).unwrap();
        assert_eq!(ids.0, vec!["a", "b", "c"]);
    }

    #[test]
    fn comma_allows_empty_when_min_is_0() {
        let json = r#""""#;
        let ids: TestCommaIds = serde_json::from_str(json).unwrap();
        assert!(ids.0.is_empty());
    }

    #[test]
    fn comma_rejects_over_max() {
        let json = r#""a,b,c,d,e,f""#;
        let result: Result<TestCommaIds, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn comma_display_formats_correctly() {
        let ids = TestCommaIds(vec!["a".to_string(), "b".to_string()]);
        assert_eq!(ids.to_string(), "a, b");
    }

    #[test]
    fn comma_serialize_as_string() {
        let ids = TestCommaIds(vec!["x".to_string(), "y".to_string()]);
        let json = serde_json::to_string(&ids).unwrap();
        assert_eq!(json, r#""x,y""#);
    }

    // === json_array tests ===

    #[test]
    fn json_deserialize_valid() {
        let json = r#"["a", "b", "c"]"#;
        let ids: TestJsonIds = serde_json::from_str(json).unwrap();
        assert_eq!(ids.0, vec!["a", "b", "c"]);
    }

    #[test]
    fn json_allows_empty_when_min_is_0() {
        let json = r#"[]"#;
        let ids: TestJsonIds = serde_json::from_str(json).unwrap();
        assert!(ids.0.is_empty());
    }

    #[test]
    fn json_rejects_over_max() {
        let json = r#"["a","b","c","d","e","f"]"#;
        let result: Result<TestJsonIds, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn json_serialize_as_array() {
        let ids = TestJsonIds(vec!["x".to_string(), "y".to_string()]);
        let json = serde_json::to_string(&ids).unwrap();
        assert_eq!(json, r#"["x","y"]"#);
    }
}
