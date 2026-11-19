//! Mirrors reference/parser/ParseFiles.java (readContents without resolution; model JSON + messages).
use sha2::{Digest, Sha256};
use swagger_parser::core::ParseOptions;
use swagger_parser::models::{json31_pretty, json_pretty, Mapper, OpenAPI};
use swagger_parser::parser::OpenAPIV3Parser;

fn java_list(v: &Option<Vec<String>>) -> String {
    match v {
        None => "null".into(),
        Some(v) => format!("[{}]", v.join(", ")),
    }
}

fn main() {
    let list = std::env::args().nth(1).expect("file list");
    let mode = std::env::args().nth(2).unwrap_or_default();
    let text = mode == "text";
    let rt = mode == "rt" || std::env::args().nth(3).is_some();
    for f in std::fs::read_to_string(list).unwrap().lines() {
        let s = String::from_utf8_lossy(&std::fs::read(f).unwrap()).to_string();
        let res = OpenAPIV3Parser::new().read_contents(&s, None, Some(&ParseOptions::default()));
        let model = match &res.openapi {
            None => "null".to_string(),
            Some(o) => if res.openapi31 { json31_pretty(o) } else { json_pretty(o) }.unwrap_or_else(|| "null".into()),
        };
        let mut model = model;
        if rt && res.openapi.is_some() {
            let m = if res.openapi31 { Mapper::Json31 } else { Mapper::Json30 };
            let tree = jackson::json::read_tree(&model, Default::default()).unwrap();
            model = match OpenAPI::from_tree(&tree, m) {
                Ok(back) => if res.openapi31 { json31_pretty(&back) } else { json_pretty(&back) }.unwrap_or_else(|| "null".into()),
                Err(e) => format!("THROW {}", e.message),
            };
        }
        let r = format!("{model}\nMESSAGES {}", java_list(&res.messages));
        if text {
            println!("=== {f}\n{r}");
            continue;
        }
        let d = Sha256::digest(r.as_bytes());
        println!("{} {}", d.iter().map(|b| format!("{b:02x}")).collect::<String>(), f);
    }
}
