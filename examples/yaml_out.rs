//! Mirrors reference/parser/YamlOut.java.
use sha2::{Digest, Sha256};
use swagger_parser::core::ParseOptions;
use swagger_parser::models::{yaml31_pretty, yaml_pretty};
use swagger_parser::parser::OpenAPIV3Parser;

fn main() {
    let list = std::env::args().nth(1).expect("file list");
    let text = std::env::args().nth(2).as_deref() == Some("text");
    swagger_parser::parser::reference::add_http_reader(std::rc::Rc::new(|uri: &str, _| {
        uri.starts_with("http://example.com/custom").then(|| {
            Ok("openapi: 3.1.0\ninfo:\n  title: Domain\n  version: '1.0'\ncomponents:\n  pathItems:\n    ExternalRef:\n      get:\n        description: ExternalRef domain\n        operationId: ExternalRef PathItem\n        responses:\n          '200':\n            description: OK".to_string())
        })
    }));
    if let Ok(cp) = std::env::var("SP_CLASSPATH") {
        swagger_parser::java::classpath::set_classpath_roots(cp.split(':').map(str::to_string).collect());
    }
    for f in std::fs::read_to_string(list).unwrap().lines() {
        let o = ParseOptions { resolve: true, ..Default::default() };
        let res = OpenAPIV3Parser::new().read_location(f, None, Some(&o));
        let r = match &res.openapi {
            None => "null".to_string(),
            Some(o) => if res.openapi31 { yaml31_pretty(o) } else { yaml_pretty(o) }.unwrap_or_else(|| "null".into()),
        };
        if text {
            println!("=== {f}\n{r}");
            continue;
        }
        let d = Sha256::digest(r.as_bytes());
        println!("{} {}", d.iter().map(|b| format!("{b:02x}")).collect::<String>(), f);
    }
}
