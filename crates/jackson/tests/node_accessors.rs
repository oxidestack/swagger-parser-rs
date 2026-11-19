//! `asText(String)`, `asInt(int)` and `asBoolean(boolean)` compared with Jackson 2.22.1 (fixtures recorded by
//! reference/jackson/NodeAccessorCases.java).

use jackson::json::{ReadFeatures, read_tree};

fn unquote(line: &str) -> String {
    read_tree(line, ReadFeatures::default()).unwrap().text_value().unwrap().to_string()
}

#[test]
fn accessors_with_defaults_match_jackson() {
    let cases = include_str!("fixtures/node-accessor-cases.txt");
    let expected = include_str!("fixtures/node-accessor-expected.txt");
    let mut failures = Vec::new();
    for (c, e) in cases.lines().zip(expected.lines()) {
        let n = read_tree(&unquote(c), ReadFeatures::default()).unwrap();
        let got = format!(
            "{}|{}|{}|{}|{}|{}",
            n.as_text_or("dflt"),
            n.as_int_or(-7),
            n.as_boolean_or(true),
            n.as_boolean_or(false),
            n.as_int(),
            n.as_boolean()
        );
        let want = unquote(e);
        if got != want {
            failures.push(format!("input {c}\n  want {want:?}\n  got  {got:?}"));
        }
    }
    assert!(failures.is_empty(), "{} mismatches:\n{}", failures.len(), failures.join("\n"));
}
