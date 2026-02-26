use std::fmt::Write;

use neo4rs::{BoltList, BoltMap, BoltString, BoltType};

/// Our own `Query` type that mirrors `neo4rs::Query` but exposes
/// `cypher()` and `params_map()` for logging and tracing.
#[derive(Clone)]
pub struct Query {
    cypher: String,
    params: BoltMap,
}

impl Query {
    pub fn new(cypher: impl Into<String>) -> Self {
        Self {
            cypher: cypher.into(),
            params: BoltMap::default(),
        }
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

    pub fn cypher(&self) -> &str {
        &self.cypher
    }

    pub fn params_map(&self) -> &BoltMap {
        &self.params
    }

    /// Returns the cypher string with `$param` placeholders replaced by their
    /// literal values, ready to copy-paste into a Neo4j browser.
    pub fn to_cypher_populated(&self) -> String {
        populate_cypher(&self.cypher, &self.params)
    }
}

/// Replaces `$param` placeholders in `cypher` with literal values from `params`.
pub fn populate_cypher(cypher: &str, params: &BoltMap) -> String {
    let mut out = cypher.to_owned();
    // Sort keys by length descending so `$skip` is replaced before a
    // hypothetical `$s`, avoiding partial substitutions.
    let mut entries: Vec<_> = params.value.iter().collect();
    entries.sort_by(|a, b| b.0.value.len().cmp(&a.0.value.len()));
    for (k, v) in entries {
        let placeholder = format!("${}", k.value);
        let literal = bolt_to_cypher_literal(v);
        out = out.replace(&placeholder, &literal);
    }
    out
}

/// Format a `BoltType` value as a Neo4j cypher literal.
fn bolt_to_cypher_literal(val: &BoltType) -> String {
    match val {
        BoltType::String(s) => format!("'{}'", s.value.replace('\\', "\\\\").replace('\'', "\\'")),
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

/// Drop-in replacement for `neo4rs::query()`
pub fn query(cypher: &str) -> Query {
    Query::new(cypher)
}
