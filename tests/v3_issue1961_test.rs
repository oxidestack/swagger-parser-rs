//! Port of `io.swagger.v3.parser.test.Issue1961Test`.

mod common;

use swagger_parser::core::ParseOptions;
use swagger_parser::java::path::JavaPath;
use swagger_parser::parser::OpenAPIV3Parser;

const ROOT_DOCUMENT: &str = "src/test/resources/issue-1961/TestCase.yaml";

fn resolve_options() -> ParseOptions {
    let mut o = ParseOptions::new();
    o.resolve = true;
    o
}

/// Issue #1961: a reference cycle back to the root file must not duplicate schemas
#[test]
fn external_schema_referencing_root_schema_does_not_create_duplicate() {
    let _g = common::setup(common::V3_MODULE);
    let result = OpenAPIV3Parser::new().read_location(ROOT_DOCUMENT, None, Some(&resolve_options()));

    assert!(result.openapi.is_some());
    let messages = result.messages.clone().unwrap_or_default();
    assert!(messages.is_empty(), "Unexpected parser messages: {}", common::java_list(&messages));

    let open_api = result.openapi.unwrap();
    let components = open_api.get_components().expect("components");
    let schemas = components.get_schemas().expect("schemas");

    assert!(schemas.contains_key("TestCase_TestCase"));
    assert!(schemas.contains_key("TestCase_v1_TestCase"));
    assert!(schemas.contains_key("TestCase_Foo"));
    assert!(schemas.contains_key("TestCase_FooType"));
    assert!(!schemas.contains_key("TestCase_Foo_1"), "The back-reference to the root file must reuse TestCase_Foo");
    assert_eq!(schemas.len(), 4);

    let root = schemas.get("TestCase_TestCase").unwrap();
    assert!(root.is_composed_schema());
    assert_eq!(root.get_any_of().unwrap().get(0).unwrap().get_ref().as_deref(), Some("#/components/schemas/TestCase_v1_TestCase"));

    let versioned = schemas.get("TestCase_v1_TestCase").unwrap();
    assert_eq!(versioned.get_properties().unwrap().get("Foo").unwrap().get_ref().as_deref(), Some("#/components/schemas/TestCase_Foo"));

    let foo = schemas.get("TestCase_Foo").unwrap();
    let foo_types = foo.get_properties().unwrap().get("FooTypes").unwrap();
    assert!(foo_types.is_array_schema());
    assert_eq!(foo_types.get_items().unwrap().get_ref().as_deref(), Some("#/components/schemas/TestCase_FooType"));
}

/// Issue #1961 control: duplicate promotion only occurs during resolution
#[test]
fn unresolved_document_keeps_original_schemas_and_references() {
    let _g = common::setup(common::V3_MODULE);
    let result = OpenAPIV3Parser::new().read_location(ROOT_DOCUMENT, None, Some(&ParseOptions::new()));

    assert!(result.openapi.is_some());
    let messages = result.messages.clone().unwrap_or_default();
    assert!(messages.is_empty(), "Unexpected parser messages: {}", common::java_list(&messages));

    let schemas = result.openapi.unwrap().get_components().unwrap().get_schemas().unwrap();
    assert_eq!(schemas.len(), 3);
    assert!(!schemas.contains_key("TestCase_v1_TestCase"));
    assert!(!schemas.contains_key("TestCase_Foo_1"));

    let root = schemas.get("TestCase_TestCase").unwrap();
    assert!(root.is_composed_schema());
    assert_eq!(root.get_any_of().unwrap().get(0).unwrap().get_ref().as_deref(), Some("./TestCase.v1.yaml#/components/schemas/TestCase_v1_TestCase"));

    let foo = schemas.get("TestCase_Foo").unwrap();
    let foo_types = foo.get_properties().unwrap().get("FooTypes").unwrap();
    assert!(foo_types.is_array_schema());
    assert_eq!(foo_types.get_items().unwrap().get_ref().as_deref(), Some("./TestCase.yaml#/components/schemas/TestCase_FooType"));
}

/// Issue #1961: file: URI as root document resolves parent directory and deduplicates schemas
#[test]
fn file_uri_root_resolves_external_refs_and_deduplicates_schemas() {
    let _g = common::setup(common::V3_MODULE);
    let file_uri = JavaPath::new(ROOT_DOCUMENT).to_absolute_path().normalize().to_uri();

    let result = OpenAPIV3Parser::new().read_location(&file_uri, None, Some(&resolve_options()));

    assert!(result.openapi.is_some());
    let messages = result.messages.clone().unwrap_or_default();
    assert!(messages.is_empty(), "Unexpected parser messages: {}", common::java_list(&messages));

    let schemas = result.openapi.unwrap().get_components().unwrap().get_schemas().expect("schemas");
    assert!(!schemas.contains_key("TestCase_Foo_1"), "file: URI root must not duplicate schemas from back-references");
    assert_eq!(schemas.len(), 4);
}
