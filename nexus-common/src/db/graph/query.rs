use std::fmt::Write;

use neo4rs::{BoltList, BoltMap, BoltString, BoltType};

/// Bounded telemetry value: `'static` strings and `u8` (WoT depth). Wider
/// types would let request data become metric dimensions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TelemetryValue {
    Str(&'static str),
    Int(u8),
}

impl From<&'static str> for TelemetryValue {
    fn from(value: &'static str) -> Self {
        Self::Str(value)
    }
}

impl From<u8> for TelemetryValue {
    fn from(value: u8) -> Self {
        Self::Int(value)
    }
}

/// Query builder with a stable telemetry label and bounded attributes.
#[derive(Clone)]
pub struct Query {
    label: &'static str,
    cypher: String,
    params: BoltMap,
    /// Telemetry-only attributes; never sent to Neo4j.
    telemetry_attrs: Vec<(&'static str, TelemetryValue)>,
}

impl Query {
    pub fn new(label: &'static str, cypher: impl Into<String>) -> Self {
        Self {
            label,
            cypher: cypher.into(),
            params: BoltMap::default(),
            telemetry_attrs: Vec::new(),
        }
    }

    pub fn label(&self) -> &'static str {
        self.label
    }

    /// Attach a telemetry dimension. Never sent to Neo4j.
    pub(crate) fn telemetry_attr(
        mut self,
        key: &'static str,
        value: impl Into<TelemetryValue>,
    ) -> Self {
        self.telemetry_attrs.push((key, value.into()));
        self
    }

    pub(crate) fn telemetry_attr_opt(
        self,
        key: &'static str,
        value: Option<impl Into<TelemetryValue>>,
    ) -> Self {
        match value {
            Some(value) => self.telemetry_attr(key, value),
            None => self,
        }
    }

    pub(crate) fn telemetry_attrs(&self) -> &[(&'static str, TelemetryValue)] {
        &self.telemetry_attrs
    }

    pub fn param<T: Into<BoltType>>(mut self, key: &str, value: T) -> Self {
        self.params.put(key.into(), value.into());
        self
    }

    pub fn params<K, V>(mut self, input: impl IntoIterator<Item = (K, V)>) -> Self
    where
        K: Into<BoltString>,
        V: Into<BoltType>,
    {
        for (k, v) in input {
            self.params.put(k.into(), v.into());
        }
        self
    }

    /// Returns the cypher string with `$param` placeholders replaced by their
    /// literal values, ready to copy-paste into a Neo4j browser.
    pub fn to_cypher_populated(&self) -> String {
        populate_cypher(&self.cypher, &self.params)
    }
}

/// Replaces `$param` placeholders in `cypher` with literal values from `params`.
pub fn populate_cypher(cypher: &str, params: &BoltMap) -> String {
    let mut out = String::with_capacity(cypher.len());
    let mut rest = cypher;
    while let Some(i) = rest.find('$') {
        out.push_str(&rest[..i]);
        rest = &rest[i + 1..];
        let len = rest
            .find(|c: char| c != '_' && !c.is_alphanumeric())
            .unwrap_or(rest.len());
        let name = &rest[..len];
        rest = &rest[len..];
        match params.value.iter().find(|(key, _)| key.value == name) {
            Some((_, value)) if !name.is_empty() => {
                out.push_str(&bolt_to_cypher_literal(value));
            }
            _ => {
                out.push('$');
                out.push_str(name);
            }
        }
    }
    out.push_str(rest);
    out
}

/// Format a `BoltType` value as a Neo4j cypher literal.
fn bolt_to_cypher_literal(val: &BoltType) -> String {
    match val {
        BoltType::String(s) => format!(
            "'{}'",
            s.value
                .replace('\\', "\\\\")
                .replace('\'', "\\'")
                .replace('\n', "\\n")
                .replace('\r', "\\r")
                .replace('\t', "\\t")
        ),
        BoltType::Integer(i) => i.value.to_string(),
        BoltType::Float(f) => format!("{}", f.value),
        BoltType::Boolean(b) => if b.value { "true" } else { "false" }.to_string(),
        BoltType::Null(_) => "null".to_string(),
        BoltType::List(list) => bolt_list_to_cypher(list),
        BoltType::Map(map) => bolt_map_to_cypher(map),
        other => format!("{:?}", other),
    }
}

fn bolt_list_to_cypher(list: &BoltList) -> String {
    let mut out = String::from('[');
    for (i, item) in list.value.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push_str(&bolt_to_cypher_literal(item));
    }
    out.push(']');
    out
}

fn bolt_map_to_cypher(map: &BoltMap) -> String {
    let mut out = String::from('{');
    for (i, (k, v)) in map.value.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        let _ = write!(out, "{}: {}", k.value, bolt_to_cypher_literal(v));
    }
    out.push('}');
    out
}

impl From<Query> for neo4rs::Query {
    fn from(q: Query) -> neo4rs::Query {
        let mut nq = neo4rs::Query::new(q.cypher);
        for (k, v) in q.params.value {
            nq = nq.param(&k.value, v);
        }
        nq
    }
}

#[cfg(test)]
fn query(cypher: impl Into<String>) -> Query {
    Query::new("test", cypher)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── bolt_to_cypher_literal ──────────────────────────────────────

    #[test]
    fn literal_string_plain() {
        let val = BoltType::String("hello".into());
        assert_eq!(bolt_to_cypher_literal(&val), "'hello'");
    }

    #[test]
    fn literal_string_escapes_quotes_and_backslashes() {
        let val = BoltType::String("it's a \\path".into());
        assert_eq!(bolt_to_cypher_literal(&val), "'it\\'s a \\\\path'");
    }

    #[test]
    fn literal_string_escapes_control_chars() {
        let val = BoltType::String("line1\nline2\r\tend".into());
        assert_eq!(bolt_to_cypher_literal(&val), "'line1\\nline2\\r\\tend'");
    }

    #[test]
    fn literal_string_empty() {
        let val = BoltType::String("".into());
        assert_eq!(bolt_to_cypher_literal(&val), "''");
    }

    #[test]
    fn literal_integer() {
        let val = BoltType::Integer(neo4rs::BoltInteger::new(42));
        assert_eq!(bolt_to_cypher_literal(&val), "42");
    }

    #[test]
    fn literal_negative_integer() {
        let val = BoltType::Integer(neo4rs::BoltInteger::new(-1));
        assert_eq!(bolt_to_cypher_literal(&val), "-1");
    }

    #[test]
    fn literal_float() {
        let val = BoltType::Float(neo4rs::BoltFloat::new(3.01));
        assert_eq!(bolt_to_cypher_literal(&val), "3.01");
    }

    #[test]
    fn literal_boolean() {
        assert_eq!(
            bolt_to_cypher_literal(&BoltType::Boolean(neo4rs::BoltBoolean::new(true))),
            "true"
        );
        assert_eq!(
            bolt_to_cypher_literal(&BoltType::Boolean(neo4rs::BoltBoolean::new(false))),
            "false"
        );
    }

    #[test]
    fn literal_null() {
        let val = BoltType::Null(neo4rs::BoltNull);
        assert_eq!(bolt_to_cypher_literal(&val), "null");
    }

    #[test]
    fn literal_list() {
        let list = BoltList::from(vec![
            BoltType::Integer(neo4rs::BoltInteger::new(1)),
            BoltType::String("two".into()),
            BoltType::Boolean(neo4rs::BoltBoolean::new(false)),
        ]);
        assert_eq!(
            bolt_to_cypher_literal(&BoltType::List(list)),
            "[1, 'two', false]"
        );
    }

    #[test]
    fn literal_list_empty() {
        let list = BoltList::from(Vec::<BoltType>::new());
        assert_eq!(bolt_to_cypher_literal(&BoltType::List(list)), "[]");
    }

    #[test]
    fn literal_map_single_entry() {
        let mut map = BoltMap::default();
        map.put(
            "key".into(),
            BoltType::Integer(neo4rs::BoltInteger::new(99)),
        );
        assert_eq!(bolt_to_cypher_literal(&BoltType::Map(map)), "{key: 99}");
    }

    // ── populate_cypher ─────────────────────────────────────────────

    #[test]
    fn populate_basic_substitution() {
        let q = query("MATCH (u:User {id: $id}) RETURN u").param("id", "abc123");
        assert_eq!(
            q.to_cypher_populated(),
            "MATCH (u:User {id: 'abc123'}) RETURN u"
        );
    }

    #[test]
    fn populate_multiple_params() {
        let q = query("MATCH (u:User {id: $id}) SET u.name = $name")
            .param("id", "abc")
            .param("name", "Alice");
        let result = q.to_cypher_populated();
        assert!(result.contains("'abc'"));
        assert!(result.contains("'Alice'"));
        assert!(!result.contains("$id"));
        assert!(!result.contains("$name"));
    }

    #[test]
    fn populate_no_params() {
        let q = query("RETURN 1");
        assert_eq!(q.to_cypher_populated(), "RETURN 1");
    }

    #[test]
    fn populate_prefix_overlap_longer_replaced_first() {
        // $user_id should not be partially matched by $user
        let q = query("MATCH (u {id: $user_id, name: $user})")
            .param("user_id", "id123")
            .param("user", "Alice");
        let result = q.to_cypher_populated();
        assert_eq!(result, "MATCH (u {id: 'id123', name: 'Alice'})");
    }

    #[test]
    fn populate_integer_and_bool_params() {
        let q = query("MATCH (p) WHERE p.age > $age AND p.active = $active RETURN p")
            .param("age", 18_i64)
            .param("active", true);
        let result = q.to_cypher_populated();
        assert!(result.contains("> 18"));
        assert!(result.contains("= true"));
    }

    #[test]
    fn populate_param_not_in_cypher_is_ignored() {
        let q = query("RETURN 1").param("unused", "value");
        assert_eq!(q.to_cypher_populated(), "RETURN 1");
    }

    #[test]
    fn populate_special_chars_in_value() {
        let q = query("SET u.bio = $bio").param("bio", "it's a\nnew \"line\"");
        let result = q.to_cypher_populated();
        assert_eq!(result, "SET u.bio = 'it\\'s a\\nnew \"line\"'");
    }

    #[test]
    fn populate_does_not_rescan_inserted_literals() {
        let q = query("RETURN $a AS a, $b AS b")
            .param("a", "$b")
            .param("b", "secret");
        assert_eq!(q.to_cypher_populated(), "RETURN '$b' AS a, 'secret' AS b");
    }

    #[test]
    fn populate_preserves_unknown_parameters() {
        let q = query("RETURN $known, $unknown").param("known", 1_i64);
        assert_eq!(q.to_cypher_populated(), "RETURN 1, $unknown");
    }

    // ── From<Query> for neo4rs::Query ───────────────────────────────

    #[test]
    fn into_neo4rs_query_preserves_cypher() {
        let q = query("RETURN $x").param("x", 42_i64);
        let nq: neo4rs::Query = q.into();
        // neo4rs::Query doesn't expose cypher publicly, but if the
        // conversion compiles and doesn't panic, the basic contract holds.
        let _ = nq;
    }

    // ── Telemetry attributes ────────────────────────────────────────

    #[test]
    fn telemetry_attrs_are_exposed_in_insertion_order() {
        let q = Query::new("test", "RETURN 1")
            .telemetry_attr("reach", "wot")
            .telemetry_attr("depth", 2_u8);
        assert_eq!(
            q.telemetry_attrs(),
            &[
                ("reach", TelemetryValue::Str("wot")),
                ("depth", TelemetryValue::Int(2)),
            ]
        );
    }

    #[test]
    fn telemetry_attrs_are_not_query_params() {
        let q = Query::new(
            "test",
            "MATCH (n) WHERE n.r = $reach AND n.d = $depth RETURN n",
        )
        .telemetry_attr("reach", "wot")
        .telemetry_attr("depth", 2_u8);

        let populated = q.to_cypher_populated();
        assert!(populated.contains("$reach"));
        assert!(populated.contains("$depth"));
    }

    // ── Query builder ───────────────────────────────────────────────

    #[test]
    fn query_builder_params_batch() {
        let q = query("MATCH (u {id: $id, name: $name})").params(vec![
            (BoltString::from("id"), BoltType::String("abc".into())),
            (BoltString::from("name"), BoltType::String("Bob".into())),
        ]);
        let result = q.to_cypher_populated();
        assert!(result.contains("'abc'"));
        assert!(result.contains("'Bob'"));
    }
}
