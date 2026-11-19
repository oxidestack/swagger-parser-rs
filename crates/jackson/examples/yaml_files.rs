//! Mirrors reference/jackson/YamlFiles.java.
use sha2::{Digest, Sha256};
fn main() {
    let list = std::env::args().nth(1).expect("file list");
    let text = std::env::args().nth(2).is_some();
    for f in std::fs::read_to_string(list).unwrap().lines() {
        let s = String::from_utf8_lossy(&std::fs::read(f).unwrap()).to_string();
        let r = match jackson::yaml::read_tree(&s, jackson::yaml::YamlReadFeatures::swagger_core()) {
            Ok(n) => n.to_java_string(),
            Err(e) => format!("ERR {}", e.message),
        };
        if text {
            println!("{f}\n{r}");
            continue;
        }
        let d = Sha256::digest(r.as_bytes());
        println!("{} {}", d.iter().map(|b| format!("{b:02x}")).collect::<String>(), f);
    }
}
