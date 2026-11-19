//! Mirrors reference/converter/ConvertFiles.java (SwaggerConverter / OpenAPIParser; model JSON + messages).
use sha2::{Digest, Sha256};
use swagger_parser::converter::SwaggerConverter;
use swagger_parser::core::ParseOptions;
use swagger_parser::models::{json31_pretty, json_pretty};
use swagger_parser::OpenAPIParser;

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
    let (entry, opts) = mode.split_once('-').expect("entry-opts");
    std::panic::set_hook(Box::new(|_| {}));
    if let Ok(cp) = std::env::var("SP_CLASSPATH") {
        swagger_parser::java::classpath::set_classpath_roots(cp.split(':').map(str::to_string).collect());
    }
    for f in std::fs::read_to_string(list).unwrap().lines() {
        let mut o = ParseOptions::default();
        if opts != "plain" {
            o.resolve = true;
        }
        if opts == "full" {
            o.resolve_fully = true;
            o.resolve_combinators = true;
            o.skip_matches = true;
            o.allow_empty_strings = true;
            o.camel_case_flatten_naming = true;
        }
        if opts == "flatten" {
            o.flatten = true;
        }
        let o = if opts == "none" { None } else { Some(o) };
        let r = std::panic::catch_unwind(|| {
            let res = match entry {
                "conv" => SwaggerConverter::new().try_read_location(f, None, o.as_ref()),
                "contents" => SwaggerConverter::new().try_read_contents(&std::fs::read_to_string(f).unwrap(), None, o.as_ref()),
                _ => OpenAPIParser::new().try_read_location(f, None, o.as_ref()),
            };
            match res {
                Err(e) => format!("THROW {}", e.to_java_string()),
                Ok(None) => "RESULT null".to_string(),
                Ok(Some(res)) => {
                    let model = match &res.openapi {
                        None => "null".to_string(),
                        Some(o) => if res.openapi31 { json31_pretty(o) } else { json_pretty(o) }.unwrap_or_else(|| "null".into()),
                    };
                    format!("{model}\nMESSAGES {}", java_list(&res.messages))
                }
            }
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
