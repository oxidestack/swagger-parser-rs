//! Class path roots that are jars, beside directories (not a port: the JVM's own class loader). One test, because the
//! roots are process-wide.

use std::io::Write;
use swagger_parser::java::classpath::{find_class_resource, resources_with_extension, set_classpath_roots};

#[test]
fn directories_and_jars_are_searched_in_class_path_order() {
    let base = std::env::temp_dir().join(format!("swagger-parser-classpath-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(base.join("classes/specs")).unwrap();
    std::fs::write(base.join("classes/specs/a.json"), "from the directory").unwrap();
    let jar = base.join("resources.jar");
    let mut writer = zip::ZipWriter::new(std::fs::File::create(&jar).unwrap());
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    writer.add_directory("specs/", options).unwrap();
    for (name, text) in [("specs/a.json", "from the jar"), ("specs/b.json", "only in the jar"), ("specs/c.yaml", "yaml")] {
        writer.start_file(name, options).unwrap();
        writer.write_all(text.as_bytes()).unwrap();
    }
    writer.finish().unwrap();
    let root = |p: &str| base.join(p).to_string_lossy().into_owned();
    set_classpath_roots(vec![root("missing"), root("classes"), root("resources.jar")]);

    let read = |name: &str| find_class_resource(name).map(|p| p.read_to_string().unwrap());
    assert_eq!(read("specs/a.json").as_deref(), Some("from the directory"));
    assert_eq!(read("/specs/b.json").as_deref(), Some("only in the jar"));
    assert_eq!(find_class_resource("specs/b.json").unwrap().read_bytes().unwrap(), b"only in the jar");
    assert_eq!(read("specs/"), None);
    assert_eq!(read("specs/missing.json"), None);
    assert_eq!(resources_with_extension("json"), ["specs/a.json", "specs/a.json", "specs/b.json"]);

    set_classpath_roots(vec![root("resources.jar"), root("classes")]);
    assert_eq!(read("specs/a.json").as_deref(), Some("from the jar"));
    set_classpath_roots(Vec::new());
    let _ = std::fs::remove_dir_all(&base);
}
