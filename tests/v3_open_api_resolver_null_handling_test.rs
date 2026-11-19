//! Port of `io.swagger.v3.parser.test.OpenAPIResolverNullHandlingTest`: `example: null` / `default: null` through
//! reference resolution.

mod common;

use swagger_parser::core::ParseOptions;
use swagger_parser::models::*;
use swagger_parser::parser::OpenAPIV3Parser;

const HEAD: &str = "openapi: 3.0.0\ninfo:\n  title: Test\n  version: 1.0.0\n";

fn options(resolve: bool, resolve_fully: bool, resolve_combinators: Option<bool>) -> ParseOptions {
    let mut o = ParseOptions::new();
    if resolve {
        o.resolve = true;
    }
    o.resolve_fully = resolve_fully;
    if let Some(c) = resolve_combinators {
        o.resolve_combinators = c;
    }
    o
}

fn read(yaml: &str, options: &ParseOptions) -> OpenAPI {
    OpenAPIV3Parser::new().read_contents(yaml, None, Some(options)).openapi.expect("openAPI")
}

fn schema(open_api: &OpenAPI, name: &str) -> Schema {
    open_api.get_components().unwrap().get_schemas().unwrap().get(name).expect(name)
}

#[test]
fn test_schema_with_explicit_null_example() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = format!("{HEAD}paths:\n  /test:\n    get:\n      responses:\n        '200':\n          description: Success\n          content:\n            application/json:\n              schema:\n                $ref: '#/components/schemas/TestSchema'\ncomponents:\n  schemas:\n    TestSchema:\n      type: object\n      example: null\n      properties:\n        id:\n          type: string\n        value:\n          type: string\n          example: null\n");
    let open_api = read(&yaml, &options(true, true, None));
    let test_schema = schema(&open_api, "TestSchema");
    assert!(test_schema.get_example().is_none());
    let value_property = test_schema.get_properties().unwrap().get("value").expect("value");
    assert!(value_property.get_example().is_none());
    let response_schema = open_api.get_paths().unwrap().get("/test").unwrap().get_get().unwrap().get_responses().unwrap().get("200").unwrap().get_content().unwrap().get("application/json").unwrap().get_schema().unwrap();
    assert!(response_schema.get_example().is_none());
}

#[test]
fn test_schema_with_explicit_null_default() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = format!("{HEAD}paths:\n  /test:\n    post:\n      requestBody:\n        content:\n          application/json:\n            schema:\n              $ref: '#/components/schemas/TestSchema'\n      responses:\n        '200':\n          description: Success\ncomponents:\n  schemas:\n    TestSchema:\n      type: object\n      default: null\n      properties:\n        status:\n          type: string\n          default: null\n        count:\n          type: integer\n          default: null\n");
    let open_api = read(&yaml, &options(true, true, None));
    let test_schema = schema(&open_api, "TestSchema");
    assert!(test_schema.get_default().is_none());
    let props = test_schema.get_properties().unwrap();
    assert!(props.get("status").expect("status").get_default().is_none());
    assert!(props.get("count").expect("count").get_default().is_none());
    let request_body_schema = open_api.get_paths().unwrap().get("/test").unwrap().get_post().unwrap().get_request_body().unwrap().get_content().unwrap().get("application/json").unwrap().get_schema().unwrap();
    assert!(request_body_schema.get_default().is_none());
}

#[test]
fn test_all_of_with_null_example_and_default() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = format!("{HEAD}paths: {{}}\ncomponents:\n  schemas:\n    BaseSchema:\n      type: object\n      example: null\n      default: null\n      properties:\n        id:\n          type: string\n    ExtendedSchema:\n      allOf:\n        - $ref: '#/components/schemas/BaseSchema'\n        - type: object\n          properties:\n            name:\n              type: string\n              example: null\n              default: null\n");
    let open_api = read(&yaml, &options(false, true, Some(true)));
    let extended = schema(&open_api, "ExtendedSchema");
    assert!(extended.get_example().is_none());
    assert!(extended.get_default().is_none());
    if let Some(props) = extended.get_properties() && let Some(name) = props.get("name") {
        assert!(name.get_example().is_none());
        assert!(name.get_default().is_none());
    }
}

#[test]
fn test_parameter_with_null_example_and_default() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = format!("{HEAD}paths:\n  /test:\n    get:\n      parameters:\n        - name: testParam\n          in: query\n          schema:\n            type: string\n            example: null\n            default: null\n        - $ref: '#/components/parameters/RefParam'\n      responses:\n        '200':\n          description: Success\ncomponents:\n  parameters:\n    RefParam:\n      name: refParam\n      in: query\n      schema:\n        type: integer\n        example: null\n        default: null\n");
    let open_api = read(&yaml, &options(true, true, None));
    let parameters = open_api.get_paths().unwrap().get("/test").unwrap().get_get().unwrap().get_parameters().expect("parameters");
    assert_eq!(2, parameters.len());

    let test_param = parameters.get(0).unwrap();
    let s = test_param.get_schema().expect("schema");
    assert!(s.get_example().is_none());
    assert!(s.get_default().is_none());

    let ref_param = parameters.get(1).unwrap();
    let s = ref_param.get_schema().expect("schema");
    assert!(s.get_example().is_none());
    assert!(s.get_default().is_none());
}

#[test]
fn test_null_default_not_propagated_in_all_of() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = format!("{HEAD}paths: {{}}\ncomponents:\n  schemas:\n    BaseSchema:\n      type: object\n      properties:\n        id:\n          type: string\n    ExtendedSchema:\n      allOf:\n        - $ref: '#/components/schemas/BaseSchema'\n        - type: object\n          properties:\n            name:\n              type: string\n");
    let open_api = read(&yaml, &options(false, true, Some(true)));
    let extended = schema(&open_api, "ExtendedSchema");
    assert!(extended.get_default().is_none());
    if let Some(props) = extended.get_properties() {
        if let Some(id) = props.get("id") {
            assert!(id.get_default().is_none());
        }
        if let Some(name) = props.get("name") {
            assert!(name.get_default().is_none());
        }
    }
}

#[test]
fn test_source_default_when_resolved_is_null() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = format!("{HEAD}paths: {{}}\ncomponents:\n  schemas:\n    BaseSchema:\n      type: object\n      properties:\n        count:\n          type: integer\n    ExtendedSchema:\n      allOf:\n        - $ref: '#/components/schemas/BaseSchema'\n      default:\n        count: 10\n");
    let open_api = read(&yaml, &options(false, true, Some(true)));
    assert!(schema(&open_api, "ExtendedSchema").get_default().is_some());
}

#[test]
fn test_one_of_null_default_not_propagated() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = format!("{HEAD}paths: {{}}\ncomponents:\n  schemas:\n    StringSchema:\n      type: string\n    NumberSchema:\n      type: number\n    UnionSchema:\n      oneOf:\n        - $ref: '#/components/schemas/StringSchema'\n        - $ref: '#/components/schemas/NumberSchema'\n");
    let open_api = read(&yaml, &options(false, true, Some(false)));
    assert!(schema(&open_api, "UnionSchema").get_default().is_none());
}

#[test]
fn test_any_of_non_null_default_preserved() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = format!("{HEAD}paths: {{}}\ncomponents:\n  schemas:\n    Schema1:\n      type: object\n      properties:\n        prop1:\n          type: string\n    Schema2:\n      type: object\n      properties:\n        prop2:\n          type: integer\n    CombinedSchema:\n      anyOf:\n        - $ref: '#/components/schemas/Schema1'\n        - $ref: '#/components/schemas/Schema2'\n      default:\n        prop1: 'test'\n");
    let open_api = read(&yaml, &options(false, true, Some(false)));
    assert!(schema(&open_api, "CombinedSchema").get_default().is_some());
}
