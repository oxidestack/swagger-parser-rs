//! Port of `io.swagger.v3.parser.test.NullHandlingTest`: "field not set" vs "field explicitly set to null".

mod common;

use jackson::{JavaObject, JsonNode};
use swagger_parser::core::ParseOptions;
use swagger_parser::models::*;
use swagger_parser::parser::OpenAPIV3Parser;

fn read(yaml: &str, options: Option<&ParseOptions>) -> OpenAPI {
    OpenAPIV3Parser::new().read_contents(yaml, None, options).openapi.expect("openAPI")
}

fn schema(open_api: &OpenAPI, name: &str) -> Schema {
    open_api.get_components().unwrap().get_schemas().unwrap().get(name).expect(name)
}

fn fully(resolve_combinators: bool) -> ParseOptions {
    let mut o = ParseOptions::new();
    o.resolve_fully = true;
    o.resolve_combinators = resolve_combinators;
    o
}

const HEAD: &str = "openapi: 3.0.0\ninfo:\n  title: Test\n  version: 1.0.0\n";

#[test]
fn test_schema_default_explicitly_null() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = format!("{HEAD}paths: {{}}\ncomponents:\n  schemas:\n    NullableSchema:\n      type: string\n      nullable: true\n      default: null\n");
    let s = schema(&read(&yaml, None), "NullableSchema");
    assert!(s.get_default().is_none(), "Default should be null");
    assert!(s.get_default_set_flag(), "Default set flag should be true");
}

#[test]
fn test_schema_default_not_set() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = format!("{HEAD}paths: {{}}\ncomponents:\n  schemas:\n    NoDefaultSchema:\n      type: string\n");
    let s = schema(&read(&yaml, None), "NoDefaultSchema");
    assert!(s.get_default().is_none(), "Default should be null");
    assert!(!s.get_default_set_flag(), "Default set flag should be false");
}

#[test]
fn test_schema_example_explicitly_null() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = format!("{HEAD}paths: {{}}\ncomponents:\n  schemas:\n    NullExampleSchema:\n      type: string\n      example: null\n");
    let s = schema(&read(&yaml, None), "NullExampleSchema");
    assert!(s.get_example().is_none(), "Example should be null");
    assert!(s.get_example_set_flag(), "Example set flag should be true");
}

#[test]
fn test_schema_example_not_set() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = format!("{HEAD}paths: {{}}\ncomponents:\n  schemas:\n    NoExampleSchema:\n      type: string\n");
    let s = schema(&read(&yaml, None), "NoExampleSchema");
    assert!(s.get_example().is_none(), "Example should be null");
    assert!(!s.get_example_set_flag(), "Example set flag should be false");
}

#[test]
fn test_example_value_explicitly_null() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = format!("{HEAD}paths: {{}}\ncomponents:\n  examples:\n    NullExample:\n      value: null\n");
    let e = read(&yaml, None).get_components().unwrap().get_examples().unwrap().get("NullExample").expect("example");
    assert!(e.get_value().is_none(), "Value should be null");
    assert!(e.get_value_set_flag(), "Value set flag should be true");
}

#[test]
fn test_example_value_not_set() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = format!("{HEAD}paths: {{}}\ncomponents:\n  examples:\n    NoValueExample:\n      summary: Example without value\n");
    let e = read(&yaml, None).get_components().unwrap().get_examples().unwrap().get("NoValueExample").expect("example");
    assert!(e.get_value().is_none(), "Value should be null");
    assert!(!e.get_value_set_flag(), "Value set flag should be false");
}

#[test]
fn test_all_of_source_default_preserved_without_resolving_combinators() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = format!("{HEAD}paths: {{}}\ncomponents:\n  schemas:\n    BaseSchema:\n      type: object\n      properties:\n        name:\n          type: string\n    ExtendedSchema:\n      allOf:\n        - $ref: '#/components/schemas/BaseSchema'\n      default:\n        name: 'default'\n");
    let extended = schema(&read(&yaml, Some(&fully(false))), "ExtendedSchema");
    assert!(extended.get_default().is_some(), "Default should be preserved when not resolving combinators");
    assert!(extended.get_default_set_flag(), "Default should be set");
}

#[test]
fn test_any_of_null_example_not_propagated() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = format!("{HEAD}paths: {{}}\ncomponents:\n  schemas:\n    Schema1:\n      type: string\n    Schema2:\n      type: number\n    CombinedSchema:\n      anyOf:\n        - $ref: '#/components/schemas/Schema1'\n        - $ref: '#/components/schemas/Schema2'\n");
    let combined = schema(&read(&yaml, Some(&fully(true))), "CombinedSchema");
    assert!(combined.get_example().is_none(), "Example should be null");
    assert!(!combined.get_example_set_flag(), "Example should not be set");
}

#[test]
fn test_one_of_explicit_null_example_preserved() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = format!("{HEAD}paths: {{}}\ncomponents:\n  schemas:\n    Schema1:\n      type: string\n    Schema2:\n      type: number\n    CombinedSchema:\n      oneOf:\n        - $ref: '#/components/schemas/Schema1'\n        - $ref: '#/components/schemas/Schema2'\n      example: null\n");
    let combined = schema(&read(&yaml, Some(&fully(true))), "CombinedSchema");
    assert!(combined.get_example().is_none(), "Example should be null");
    assert!(combined.get_example_set_flag(), "Example should be explicitly set");
}

#[test]
fn test_property_example_null_vs_not_set() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = format!("{HEAD}paths: {{}}\ncomponents:\n  schemas:\n    TestSchema:\n      type: object\n      properties:\n        withNullExample:\n          type: string\n          example: null\n        withoutExample:\n          type: string\n        withExample:\n          type: string\n          example: 'test'\n");
    let test_schema = schema(&read(&yaml, None), "TestSchema");
    let props = test_schema.get_properties().unwrap();

    let with_null = props.get("withNullExample").unwrap();
    assert!(with_null.get_example().is_none());
    assert!(with_null.get_example_set_flag(), "Should be explicitly set to null");

    let without = props.get("withoutExample").unwrap();
    assert!(without.get_example().is_none());
    assert!(!without.get_example_set_flag(), "Should not be set");

    let with_example = props.get("withExample").unwrap();
    assert_eq!(with_example.get_example(), Some(JavaObject::string("test")));
    assert!(with_example.get_example_set_flag(), "Should be set");
}

#[test]
fn test_parameter_schema_example_null_vs_not_set() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = format!("{HEAD}paths:\n  /test:\n    get:\n      parameters:\n        - name: nullExample\n          in: query\n          schema:\n            type: string\n            example: null\n        - name: noExample\n          in: query\n          schema:\n            type: string\n      responses:\n        '200':\n          description: OK\n");
    let open_api = read(&yaml, None);
    let params = open_api.get_paths().unwrap().get("/test").unwrap().get_get().unwrap().get_parameters().unwrap();
    assert_eq!(params.len(), 2);

    let null_schema = params.get(0).unwrap().get_schema().unwrap();
    assert!(null_schema.get_example().is_none());
    assert!(null_schema.get_example_set_flag(), "Should be explicitly set to null");

    let no_example_schema = params.get(1).unwrap().get_schema().unwrap();
    assert!(no_example_schema.get_example().is_none());
    assert!(!no_example_schema.get_example_set_flag(), "Should not be set");
}

#[test]
fn test_media_type_example_null_vs_not_set() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = format!("{HEAD}paths:\n  /test:\n    post:\n      requestBody:\n        content:\n          application/json:\n            schema:\n              type: object\n            example: null\n          application/xml:\n            schema:\n              type: object\n      responses:\n        '200':\n          description: OK\n");
    let open_api = read(&yaml, None);
    let content = open_api.get_paths().unwrap().get("/test").unwrap().get_post().unwrap().get_request_body().unwrap().get_content().unwrap();

    let json_media = content.get("application/json").unwrap();
    assert!(json_media.get_example().is_none());
    assert!(json_media.get_example_set_flag(), "Should be explicitly set to null");

    let xml_media = content.get("application/xml").unwrap();
    assert!(xml_media.get_example().is_none());
    assert!(!xml_media.get_example_set_flag(), "Should not be set");
}

#[test]
fn test_header_schema_example_null_vs_not_set() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = format!("{HEAD}paths:\n  /test:\n    get:\n      responses:\n        '200':\n          description: OK\n          headers:\n            X-Null-Example:\n              schema:\n                type: string\n                example: null\n            X-No-Example:\n              schema:\n                type: string\n");
    let open_api = read(&yaml, None);
    let headers = open_api.get_paths().unwrap().get("/test").unwrap().get_get().unwrap().get_responses().unwrap().get("200").unwrap().get_headers().unwrap();

    let null_schema = headers.get("X-Null-Example").unwrap().get_schema().unwrap();
    assert!(null_schema.get_example().is_none());
    assert!(null_schema.get_example_set_flag(), "Should be explicitly set to null");

    let no_example_schema = headers.get("X-No-Example").unwrap().get_schema().unwrap();
    assert!(no_example_schema.get_example().is_none());
    assert!(!no_example_schema.get_example_set_flag(), "Should not be set");
}

#[test]
fn test_all_of_multiple_different_defaults() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = format!("{HEAD}paths: {{}}\ncomponents:\n  schemas:\n    Schema1:\n      type: object\n      default:\n        value: 1\n      properties:\n        value:\n          type: integer\n    Schema2:\n      type: object\n      default:\n        value: 2\n      properties:\n        value:\n          type: integer\n    CombinedSchema:\n      allOf:\n        - $ref: '#/components/schemas/Schema1'\n        - $ref: '#/components/schemas/Schema2'\n");
    let combined = schema(&read(&yaml, Some(&fully(true))), "CombinedSchema");
    // When multiple different defaults exist, none should be set
    assert!(combined.get_default().is_none(), "Default should not be set when multiple different defaults exist");
}

#[test]
fn test_resolve_fully_without_combinators() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = format!("{HEAD}paths: {{}}\ncomponents:\n  schemas:\n    BaseSchema:\n      type: object\n      default:\n        name: 'test'\n      properties:\n        name:\n          type: string\n    ExtendedSchema:\n      allOf:\n        - $ref: '#/components/schemas/BaseSchema'\n      default:\n        name: 'extended'\n");
    let extended = schema(&read(&yaml, Some(&fully(false))), "ExtendedSchema");
    let default = extended.get_default();
    assert!(default.is_some(), "Default should be preserved");
    // Default is stored as a JsonNode internally: instanceof ObjectNode
    assert!(matches!(default, Some(JavaObject::Node(JsonNode::Object(_)))), "{default:?}");
}
