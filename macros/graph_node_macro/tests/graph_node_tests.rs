use graph_node_macro::GraphNode;
use neo4rs::{BoltBoolean, BoltInteger, BoltList, BoltMap, BoltNode, BoltString, BoltType, Node};

#[derive(GraphNode, PartialEq)]
struct TestStruct {
    string_field: String,
    integer_field: i64,
    boolean_field: bool,
}

#[test]
fn test_from_node_result_equals_instance() {
    let instance = TestStruct {
        integer_field: 10,
        string_field: String::from("hello world"),
        boolean_field: true,
    };
    let id = BoltInteger::new(1);
    let bolt_list = BoltList::new();
    let mut bolt_map = BoltMap::new();
    bolt_map.put(
        BoltString::new("integer_field"),
        BoltType::Integer(BoltInteger { value: 10 }),
    );
    bolt_map.put(
        BoltString::new("string_field"),
        BoltType::String(BoltString {
            value: String::from("hello world"),
        }),
    );
    bolt_map.put(
        BoltString::new("boolean_field"),
        BoltType::Boolean(BoltBoolean { value: true }),
    );
    let bolt_node = BoltNode::new(id, bolt_list, bolt_map);
    let node = Node::new(bolt_node);
    let from_node: TestStruct = TestStruct::from_node(&node);
    assert!(from_node == instance)
}
