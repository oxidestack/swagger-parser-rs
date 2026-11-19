//! Mirrors reference/parser/SchemaHashes.java.
use swagger_parser::core::ParseOptions;
use swagger_parser::models::JavaHash;
use swagger_parser::parser::OpenAPIV3Parser;

fn main() {
    let list = std::env::args().nth(1).expect("file list");
    for f in std::fs::read_to_string(list).unwrap().lines() {
        let s = String::from_utf8_lossy(&std::fs::read(f).unwrap()).to_string();
        let res = OpenAPIV3Parser::new().read_contents(&s, None, Some(&ParseOptions::default()));
        let Some(schemas) = res.openapi.and_then(|o| o.get_components()).and_then(|c| c.get_schemas()) else { continue };
        for (k, v) in schemas.entries() {
            println!("{f} {k} {}", v.java_hash_code());
        }
    }
}
