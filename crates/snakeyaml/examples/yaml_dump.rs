//! Mirrors reference/snakeyaml/YamlDump.java: typed dump of SafeConstructor output, hashed per file.
use sha2::{Digest, Sha256};
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

fn main() {
    let list = std::env::args().nth(1).expect("file list");
    let text = std::env::args().nth(2).as_deref() == Some("text");
    for f in std::fs::read_to_string(list).unwrap().lines() {
        let s = String::from_utf8_lossy(&std::fs::read(f).unwrap()).to_string();
        let r = format!("{}\n{}", run(&s, true), run(&s, false));
        if text {
            println!("{f}\n{r}");
            continue;
        }
        let d = Sha256::digest(r.as_bytes());
        println!("{} {}", d.iter().map(|b| format!("{b:02x}")).collect::<String>(), f);
    }
}
