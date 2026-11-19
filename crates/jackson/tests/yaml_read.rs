//! YAML tree reading compared with jackson-dataformat-yaml 2.22.1 (fixtures by reference/jackson/YamlTreeCases.java).

use jackson::json::{ReadFeatures, read_tree as read_json};
use jackson::node::JsonNode;
use jackson::yaml::{YamlReadFeatures, read_tree};

fn cls(n: &JsonNode) -> &'static str {
    match n {
        JsonNode::Object(_) => "ObjectNode",
        JsonNode::Array(_) => "ArrayNode",
        JsonNode::Text(_) => "TextNode",
        JsonNode::Binary(_) => "BinaryNode",
        JsonNode::Boolean(_) => "BooleanNode",
        JsonNode::Null => "NullNode",
        JsonNode::Missing => "MissingNode",
        JsonNode::Short(_) => "ShortNode",
        JsonNode::Int(_) => "IntNode",
        JsonNode::Long(_) => "LongNode",
        JsonNode::BigInteger(_) => "BigIntegerNode",
        JsonNode::Float(_) => "FloatNode",
        JsonNode::Double(_) => "DoubleNode",
        JsonNode::Decimal(_) => "DecimalNode",
        JsonNode::Pojo(_) => "POJONode",
    }
}

fn run(s: &str, f: YamlReadFeatures) -> String {
    match read_tree(s, f) {
        Ok(n) => {
            let mut out = format!("OK {} {}", cls(&n), n.to_java_string());
            if let Some(m) = n.as_object() {
                out.push_str(" T:");
                for (k, v) in m.iter() {
                    out.push_str(&format!("{k}={},", cls(v)));
                }
            }
            out
        }
        Err(e) => format!("ERR {}: {}", e.class, e.message),
    }
}

fn unquote(line: &str) -> String {
    read_json(line, ReadFeatures::default()).unwrap().text_value().unwrap().to_string()
}

#[test]
fn matches_jackson_yaml_on_recorded_cases() {
    let cases = include_str!("fixtures/yaml-tree-cases.txt");
    let expected = include_str!("fixtures/yaml-tree-expected.txt");
    let mut failures = Vec::new();
    for (c, e) in cases.lines().zip(expected.lines()) {
        let input = unquote(c);
        let want = unquote(e);
        let got = format!("{}\n{}", run(&input, YamlReadFeatures::swagger_core()), run(&input, YamlReadFeatures::swagger_parser()));
        if got != want {
            failures.push(format!("input {c}\n--- want\n{want}\n--- got\n{got}"));
        }
    }
    assert!(failures.is_empty(), "{} mismatches:\n{}", failures.len(), failures.join("\n\n"));
}
