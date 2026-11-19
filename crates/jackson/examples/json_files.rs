//! Prints `sha256(Json.pretty(tree) + "\n" + tree.toString())` per file, matching reference/jackson/JsonFiles.java.
use sha2::{Digest, Sha256};
fn main() {
    let list = std::env::args().nth(1).expect("file list");
    for f in std::fs::read_to_string(list).unwrap().lines() {
        let s = std::fs::read_to_string(f).unwrap();
        let r = match jackson::json::read_tree(&s, jackson::json::ReadFeatures { strict_duplicate_detection: true, ..Default::default() }) {
            Ok(n) => format!("{}\n{}", jackson::convert::node_to_json_string(&n, true, true), n.to_java_string()),
            Err(e) => format!("ERR {}", e.message),
        };
        let d = Sha256::digest(r.as_bytes());
        println!("{} {}", d.iter().map(|b| format!("{b:02x}")).collect::<String>(), f);
    }
}
