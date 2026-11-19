//! Port of `io.swagger.v3.parser.reference.ReferenceVisitorTest`.
//!
//! `new ReferenceVisitor(null, null, null, null)`: `deserializeIntoTree` uses none of the visitor's state, so the
//! Rust visitor gets an empty reference, a traverser over an empty context and no context. The
//! `maxYamlCodePoints` system property is the process environment variable of that name here.

mod common;

use std::rc::Rc;
use swagger_parser::core::{ParseOptions, SwaggerParseResult};
use swagger_parser::java::classpath::find_class_resource;
use swagger_parser::parser::reference::{DereferencerContext, OpenAPI31Traverser, Reference, ReferenceVisitor};

/// SnakeYAML's `YAMLException` and its subclasses (`expectedExceptions = YAMLException.class`).
const YAML_EXCEPTIONS: &[&str] = &[
    "org.yaml.snakeyaml.error.YAMLException",
    "org.yaml.snakeyaml.error.MarkedYAMLException",
    "org.yaml.snakeyaml.scanner.ScannerException",
    "org.yaml.snakeyaml.parser.ParserException",
    "org.yaml.snakeyaml.composer.ComposerException",
    "org.yaml.snakeyaml.constructor.ConstructorException",
    "org.yaml.snakeyaml.constructor.DuplicateKeyException",
    "org.yaml.snakeyaml.reader.ReaderException",
];

fn new_visitor() -> ReferenceVisitor {
    let ctx = Rc::new(DereferencerContext::new(&SwaggerParseResult::new(), None, None, &ParseOptions::default()));
    ReferenceVisitor::new(Reference::new(), Rc::new(OpenAPI31Traverser::new(ctx)), None).unwrap()
}

/// `getClass().getResourceAsStream(name)` read as UTF-8
fn read_resource_as_string(resource_name: &str) -> String {
    let p = find_class_resource(resource_name).unwrap_or_else(|| panic!("Resource not found: {resource_name}"));
    p.read_to_string().unwrap()
}

#[test]
fn large_file_should_be_parsed_by_jackson_library_when_code_limit_is_set() {
    let _g = common::setup(common::V3_MODULE);
    // System.setProperty("maxYamlCodePoints", "10000000")
    unsafe { std::env::set_var("maxYamlCodePoints", "10000000") };
    struct Clear;
    impl Drop for Clear {
        fn drop(&mut self) {
            unsafe { std::env::remove_var("maxYamlCodePoints") };
        }
    }
    let _clear = Clear;
    let visitor = new_visitor();
    let content = read_resource_as_string("/issue2059/largeFile.yaml");
    let json_node = visitor.deserialize_into_tree(Some(&content));
    assert!(!content.is_empty());
    assert!(json_node.is_ok(), "{:?}", json_node.err());
}

#[test]
fn unsafe_yaml_tag_should_be_rejected() {
    let _g = common::setup(common::V3_MODULE);
    let visitor = new_visitor();
    let e = visitor.deserialize_into_tree(Some("!!javax.script.ScriptEngineManager [!!java.net.URLClassLoader [[]]]")).unwrap_err();
    assert!(YAML_EXCEPTIONS.contains(&e.class.as_str()), "{}", e.class);
}

#[test]
fn large_file_should_not_be_parsed_by_jackson_library_when_code_limit_is_not_set() {
    let _g = common::setup(common::V3_MODULE);
    let visitor = new_visitor();
    let content = read_resource_as_string("/issue2059/largeFile.yaml");
    match visitor.deserialize_into_tree(Some(&content)) {
        Ok(_) => panic!("Expected YAMLException was not thrown"),
        Err(ex) => {
            assert!(YAML_EXCEPTIONS.contains(&ex.class.as_str()), "{}", ex.class);
            assert!(
                ex.message.as_deref().is_some_and(|m| m.contains("The incoming YAML document exceeds the limit: 3145728 code points.")),
                "Unexpected YAMLException message: {:?}",
                ex.message
            );
        }
    }
}
