//! SafeConstructor output and error messages compared with SnakeYAML 2.6 (fixtures recorded by
//! reference/snakeyaml/YamlCases.java).
use snakeyaml::{LoaderOptions, Resolver, Yaml, YamlObject};
use std::collections::HashMap;

fn dump(o: &YamlObject, sb: &mut String, seen: &mut HashMap<usize, usize>) {
    if let Some(id) = o.identity() {
        if let Some(n) = seen.get(&id) {
            sb.push_str(&format!("@{n}"));
            return;
        }
        let n = seen.len();
        seen.insert(id, n);
    }
    match o {
        YamlObject::Null => sb.push('N'),
        YamlObject::Map(m) => {
            sb.push('{');
            for (k, v) in m.entries() {
                dump(&k, sb, seen);
                sb.push('=');
                dump(&v, sb, seen);
                sb.push(',');
            }
            sb.push('}');
        }
        YamlObject::Set(s) => {
            sb.push_str("S[");
            for v in s.items() {
                dump(&v, sb, seen);
                sb.push(',');
            }
            sb.push(']');
        }
        YamlObject::List(s) => {
            sb.push('[');
            for v in s.items() {
                dump(&v, sb, seen);
                sb.push(',');
            }
            sb.push(']');
        }
        YamlObject::Pairs(p) => {
            sb.push('[');
            for (k, v) in p.iter() {
                sb.push_str("P(");
                dump(k, sb, seen);
                sb.push(':');
                dump(v, sb, seen);
                sb.push_str("),");
            }
            sb.push(']');
        }
        YamlObject::String(s) => sb.push_str(&format!("s:{};", s.replace('\\', "\\\\").replace('\n', "\\n"))),
        YamlObject::Integer(v) => sb.push_str(&format!("i:{v}")),
        YamlObject::Long(v) => sb.push_str(&format!("l:{v}")),
        YamlObject::BigInteger(v) => sb.push_str(&format!("I:{v}")),
        YamlObject::Double(v) => sb.push_str(&format!("d:{}", java_compat::double_to_string(*v))),
        YamlObject::Boolean(v) => sb.push_str(&format!("b:{v}")),
        YamlObject::Bytes(b) => {
            use base64::Engine;
            sb.push_str(&format!("B:{}", base64::engine::general_purpose::STANDARD.encode(b)))
        }
        YamlObject::Date(ms) => sb.push_str(&format!("D:{ms}")),
    }
}

fn run(s: &str, custom: bool) -> String {
    let lo = LoaderOptions {
        max_aliases_for_collections: i32::MAX as usize,
        allow_recursive_keys: true,
        allow_duplicate_keys: false,
        code_point_limit: 3 * 1024 * 1024,
        ..LoaderOptions::default()
    };
    let yaml = Yaml::new(lo, if custom { Resolver::swagger_custom() } else { Resolver::default() });
    match yaml.load(s) {
        Ok(o) => {
            let mut sb = String::new();
            dump(&o, &mut sb, &mut HashMap::new());
            sb
        }
        Err(e) => format!("ERR {}: {}", e.class, e.message()),
    }
}

#[test]
fn matches_snakeyaml_on_recorded_cases() {
    let cases = include_str!("fixtures/yaml-cases.txt");
    let expected = include_str!("fixtures/yaml-expected.txt");
    let mut failures = Vec::new();
    for (c, e) in cases.lines().zip(expected.lines()) {
        let input: String = serde_json::from_str(c).unwrap();
        let want: String = serde_json::from_str(e).unwrap();
        let got = format!("{}\n{}", run(&input, true), run(&input, false));
        if got != want {
            failures.push(format!("input {c}\n--- want\n{want}\n--- got\n{got}"));
        }
    }
    assert!(failures.is_empty(), "{} mismatches:\n{}", failures.len(), failures.join("\n\n"));
}
