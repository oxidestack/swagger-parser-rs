//! Mirrors reference/parser/ResolveFiles.java (readLocation with resolution; model JSON + messages).
use sha2::{Digest, Sha256};
use swagger_parser::core::ParseOptions;
use swagger_parser::models::{json31_pretty, json_pretty};
use swagger_parser::parser::OpenAPIV3Parser;

fn java_list(v: &Option<Vec<String>>) -> String {
    match v {
        None => "null".into(),
        Some(v) => format!("[{}]", v.join(", ")),
    }
}

fn main() {
    let list = std::env::args().nth(1).expect("file list");
    let mode = std::env::args().nth(2).expect("mode");
    let text = std::env::args().nth(3).as_deref() == Some("text");
    std::panic::set_hook(Box::new(|_| {}));
    // the v3 test resources register io.swagger.v3.parser.test.CustomOpenAPIDereferencer
    swagger_parser::parser::reference::add_http_reader(std::rc::Rc::new(|uri: &str, _| {
        uri.starts_with("http://example.com/custom").then(|| {
            Ok("openapi: 3.1.0\ninfo:\n  title: Domain\n  version: '1.0'\ncomponents:\n  pathItems:\n    ExternalRef:\n      get:\n        description: ExternalRef domain\n        operationId: ExternalRef PathItem\n        responses:\n          '200':\n            description: OK".to_string())
        })
    }));
    if let Ok(cp) = std::env::var("SP_CLASSPATH") {
        swagger_parser::java::classpath::set_classpath_roots(cp.split(':').map(str::to_string).collect());
    }
    for f in std::fs::read_to_string(list).unwrap().lines() {
        let mut o = ParseOptions { resolve: true, ..Default::default() };
        if mode == "full" {
            o.resolve_fully = true;
            o.resolve_combinators = true;
            o.skip_matches = true;
            o.allow_empty_strings = true;
            o.camel_case_flatten_naming = true;
        }
        if mode.starts_with("flatten") {
            o.flatten = true;
        }
        if mode == "flattenc" {
            o.flatten_composed_schemas = true;
            o.camel_case_flatten_naming = true;
        }
        if mode == "flattens" {
            o.skip_matches = true;
            o.resolve_fully = true;
        }
        let r = std::panic::catch_unwind(|| {
            let res = OpenAPIV3Parser::new().read_location(f, None, Some(&o));
            let model = match &res.openapi {
                None => "null".to_string(),
                Some(o) => if res.openapi31 { json31_pretty(o) } else { json_pretty(o) }.unwrap_or_else(|| "null".into()),
            };
            format!("{model}\nMESSAGES {}", java_list(&res.messages))
        })
        .unwrap_or_else(|_| "PANIC".to_string());
        if text {
            println!("=== {f}\n{r}");
            continue;
        }
        let d = Sha256::digest(r.as_bytes());
        println!("{} {}", d.iter().map(|b| format!("{b:02x}")).collect::<String>(), f);
    }
}
