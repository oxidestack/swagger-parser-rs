//! JSON parsing and writing compared with Jackson 2.22.1 (fixtures recorded by reference/jackson/JsonCases.java).

use jackson::convert::node_to_json_string;
use jackson::json::{ReadFeatures, read_tree};
use jackson::node::JsonNode;

fn node_class(n: &JsonNode) -> &'static str {
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

fn unquote(line: &str) -> String {
    read_tree(line, ReadFeatures::default()).unwrap().text_value().unwrap().to_string()
}

#[test]
fn matches_jackson_on_recorded_cases() {
    let cases = include_str!("fixtures/json-cases.txt");
    let expected = include_str!("fixtures/json-expected.txt");
    let mut failures = Vec::new();
    for (c, e) in cases.lines().zip(expected.lines()) {
        let input = unquote(c);
        let want = unquote(e);
        let got = match read_tree(&input, ReadFeatures { strict_duplicate_detection: true, ..Default::default() }) {
            Ok(n) => format!("OK {} {} PRETTY {}", node_class(&n), n.to_java_string(), node_to_json_string(&n, true, true)),
            Err(e) => format!("ERR {}: {}", e.class, e.message),
        };
        if got != want {
            failures.push(format!("input {c}\n  want {want:?}\n  got  {got:?}"));
        }
    }
    assert!(failures.is_empty(), "{} mismatches:\n{}", failures.len(), failures.join("\n"));
}

/// The parser features MockServer's `ObjectMapperFactory` turns on (comments, trailing commas, missing values, leading
/// zeros, …), recorded by reference/jackson/MockServerJsonCases.java.
#[test]
fn matches_jackson_with_mockserver_features() {
    let cases = include_str!("fixtures/mockserver-json-cases.txt");
    let expected = include_str!("fixtures/mockserver-json-expected.txt");
    let mut failures = Vec::new();
    for (c, e) in cases.lines().zip(expected.lines()) {
        let input = unquote(c);
        let want = unquote(e);
        let got = match read_tree(&input, ReadFeatures::mockserver()) {
            Ok(n) => format!("OK {} {}", node_class(&n), n.to_java_string()),
            Err(e) => format!("ERR {}: {}", e.class, e.message),
        };
        if got != want {
            failures.push(format!("input {c}\n  want {want:?}\n  got  {got:?}"));
        }
    }
    assert!(failures.is_empty(), "{} mismatches:\n{}", failures.len(), failures.join("\n"));
}

fn hex(line: &str) -> Vec<u8> {
    (0..line.len()).step_by(2).map(|i| u8::from_str_radix(&line[i..i + 2], 16).unwrap()).collect()
}

/// `new ObjectMapper().readTree(byte[])` on raw bytes (UTF-8 valid and not, BOMs, UTF-16, UTF-32), recorded by
/// reference/jackson/JsonBytesCases.java in the 9.1.0 webhook image's JVM (Jackson 2.22.3).
#[test]
fn matches_jackson_reading_bytes() {
    let cases = include_str!("fixtures/json-bytes-cases.txt");
    let expected = include_str!("fixtures/json-bytes-expected.txt");
    let mut failures = Vec::new();
    for (c, e) in cases.lines().zip(expected.lines()) {
        let want = unquote(e);
        let got = match jackson::json::read_tree_bytes(&hex(c.trim()), ReadFeatures { tree_member_values: true, ..Default::default() }) {
            Ok(n) => format!("OK {} {}", node_class(&n), n.to_java_string()),
            Err(e) => format!("ERR {}: {}", e.class, e.message),
        };
        if got != want {
            failures.push(format!("input {c}\n  want {want:?}\n  got  {got:?}"));
        }
    }
    assert!(failures.is_empty(), "{} mismatches:\n{}", failures.len(), failures.join("\n"));
}
