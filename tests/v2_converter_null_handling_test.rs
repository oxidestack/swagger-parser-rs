//! Port of `io.swagger.parser.test.V2ConverterNullHandlingTest` (swagger-parser-v2-converter).

mod common;

use jackson::JavaObject;
use swagger_parser::converter::SwaggerConverter;
use swagger_parser::models::{json_pretty, OpenAPI, Parameter, Schema};

/// `new SwaggerConverter().readContents(yaml, null, null).getOpenAPI()`
fn read_contents(yaml: &str) -> Option<OpenAPI> {
    SwaggerConverter::new().try_read_contents(yaml, None, None).expect("readContents").expect("result").openapi
}

/// `openAPI.getPaths().get(path).getGet().getParameters()`
fn get_parameters(open_api: &OpenAPI, path: &str) -> Vec<Parameter> {
    open_api.get_paths().unwrap().get(path).unwrap().get_get().unwrap().get_parameters().unwrap().to_vec()
}

/// `openAPI.getComponents().getSchemas().get(name)`
fn component_schema(open_api: &OpenAPI, name: &str) -> Option<Schema> {
    open_api.get_components().unwrap().get_schemas().unwrap().get(name)
}

#[test]
fn test_v2_parameter_without_default_not_set_in_v3() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let v2_yaml = "swagger: '2.0'\n\
                   info:\n  title: Test\n  version: 1.0.0\n\
                   paths:\n  /test:\n    get:\n      parameters:\n        - name: param1\n          in: query\n          type: string\n\
                   \x20     responses:\n        '200':\n          description: OK\n";

    let open_api = read_contents(v2_yaml);
    assert!(open_api.is_some());
    let open_api = open_api.unwrap();
    println!("{}", json_pretty(&open_api).unwrap());

    let params = get_parameters(&open_api, "/test");
    assert_eq!(params.len(), 1);

    let schema = params[0].get_schema();
    assert!(schema.is_some());
    let schema = schema.unwrap();
    assert!(schema.get_default().is_none(), "Default should be null");
    assert!(!schema.get_default_set_flag(), "Default should not be set");
}

#[test]
fn test_v2_parameter_with_default_preserved_in_v3() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let v2_yaml = "swagger: '2.0'\n\
                   info:\n  title: Test\n  version: 1.0.0\n\
                   paths:\n  /test:\n    get:\n      parameters:\n        - name: param1\n          in: query\n          type: string\n          default: 'test'\n\
                   \x20     responses:\n        '200':\n          description: OK\n";

    let open_api = read_contents(v2_yaml);
    assert!(open_api.is_some());
    let open_api = open_api.unwrap();
    println!("{}", json_pretty(&open_api).unwrap());
    let params = get_parameters(&open_api, "/test");
    assert_eq!(params.len(), 1);

    let schema = params[0].get_schema();
    assert!(schema.is_some());
    let schema = schema.unwrap();
    assert_eq!(schema.get_default(), Some(JavaObject::string("test")), "Default should be 'test'");
    assert!(schema.get_default_set_flag(), "Default should be set");
}

#[test]
fn test_v2_body_parameter_without_default_not_set_in_v3() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let v2_yaml = "swagger: '2.0'\n\
                   info:\n  title: Test\n  version: 1.0.0\n\
                   paths:\n  /test:\n    post:\n      parameters:\n        - name: body\n          in: body\n          schema:\n            type: object\n            properties:\n              name:\n                type: string\n\
                   \x20     responses:\n        '200':\n          description: OK\n";

    let open_api = read_contents(v2_yaml);
    assert!(open_api.is_some());
    let open_api = open_api.unwrap();
    println!("{}", json_pretty(&open_api).unwrap());

    let request_body = open_api.get_paths().unwrap().get("/test").unwrap().get_post().unwrap().get_request_body();
    assert!(request_body.is_some());
    let request_body = request_body.unwrap();
    assert!(request_body.get_content().is_some());
    let content = request_body.get_content().unwrap();
    assert!(!content.is_empty());

    let media_type = if content.contains_key("application/json") { "application/json" } else { "*/*" };
    let schema = content.get(media_type).unwrap().get_schema();
    assert!(schema.is_some());
    let schema = schema.unwrap();
    assert!(schema.get_default().is_none(), "Default should be null");
    assert!(!schema.get_default_set_flag(), "Default should not be set");
}

#[test]
fn test_v2_definition_without_default_not_set_in_v3() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let v2_yaml = "swagger: '2.0'\n\
                   info:\n  title: Test\n  version: 1.0.0\n\
                   paths: {}\n\
                   definitions:\n  TestModel:\n    type: object\n    properties:\n      name:\n        type: string\n";

    let open_api = read_contents(v2_yaml);
    assert!(open_api.is_some());
    let open_api = open_api.unwrap();
    println!("{}", json_pretty(&open_api).unwrap());

    let schema = component_schema(&open_api, "TestModel");
    assert!(schema.is_some());
    let schema = schema.unwrap();
    assert!(schema.get_default().is_none(), "Default should be null");
    assert!(!schema.get_default_set_flag(), "Default should not be set");
}

#[test]
fn test_v2_property_with_default_preserved_in_v3() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let v2_yaml = "swagger: '2.0'\n\
                   info:\n  title: Test\n  version: 1.0.0\n\
                   paths: {}\n\
                   definitions:\n  TestModel:\n    type: object\n    properties:\n      status:\n        type: string\n        default: 'active'\n";

    let open_api = read_contents(v2_yaml);
    assert!(open_api.is_some());
    let open_api = open_api.unwrap();
    println!("{}", json_pretty(&open_api).unwrap());

    let schema = component_schema(&open_api, "TestModel");
    assert!(schema.is_some());
    let schema = schema.unwrap();

    let status_prop = schema.get_properties().unwrap().get("status");
    assert!(status_prop.is_some());
    let status_prop = status_prop.unwrap();
    assert_eq!(status_prop.get_default(), Some(JavaObject::string("active")), "Default should be 'active'");
    assert!(status_prop.get_default_set_flag(), "Default should be set");
}

#[test]
fn test_v2_array_items_without_default_not_set_in_v3() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let v2_yaml = "swagger: '2.0'\n\
                   info:\n  title: Test\n  version: 1.0.0\n\
                   paths:\n  /test:\n    get:\n      parameters:\n        - name: items\n          in: query\n          type: array\n          items:\n            type: string\n\
                   \x20     responses:\n        '200':\n          description: OK\n";

    let open_api = read_contents(v2_yaml);
    assert!(open_api.is_some());
    let open_api = open_api.unwrap();
    println!("{}", json_pretty(&open_api).unwrap());

    let params = get_parameters(&open_api, "/test");
    assert_eq!(params.len(), 1);

    let schema = params[0].get_schema();
    assert!(schema.is_some());
    let schema = schema.unwrap();
    assert!(schema.is_array_schema());

    let array_schema = schema;
    assert!(array_schema.get_default().is_none(), "Default should be null");
    assert!(!array_schema.get_default_set_flag(), "Default should not be set");
}

#[test]
fn test_v2_example_preserved_in_v3() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let v2_yaml = "swagger: '2.0'\n\
                   info:\n  title: Test\n  version: 1.0.0\n\
                   paths: {}\n\
                   definitions:\n  TestModel:\n    type: object\n    example:\n      name: 'test'\n    properties:\n      name:\n        type: string\n";

    let open_api = read_contents(v2_yaml);
    assert!(open_api.is_some());
    let open_api = open_api.unwrap();

    let schema = component_schema(&open_api, "TestModel");
    assert!(schema.is_some());
    assert!(schema.unwrap().get_example().is_some(), "Example should be set");
}

#[test]
fn test_v2_property_without_example_not_set_in_v3() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let v2_yaml = "swagger: '2.0'\n\
                   info:\n  title: Test\n  version: 1.0.0\n\
                   paths: {}\n\
                   definitions:\n  TestModel:\n    type: object\n    properties:\n      name:\n        type: string\n";

    let open_api = read_contents(v2_yaml);
    assert!(open_api.is_some());
    let open_api = open_api.unwrap();

    let schema = component_schema(&open_api, "TestModel");
    assert!(schema.is_some());
    let schema = schema.unwrap();

    let name_prop = schema.get_properties().unwrap().get("name");
    assert!(name_prop.is_some());
    let name_prop = name_prop.unwrap();
    assert!(name_prop.get_example().is_none(), "Example should be null");
    assert!(!name_prop.get_example_set_flag(), "Example should not be set");
}

#[test]
fn test_v2_integer_parameter_with_default() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let v2_yaml = "swagger: '2.0'\n\
                   info:\n  title: Test\n  version: 1.0.0\n\
                   paths:\n  /test:\n    get:\n      parameters:\n        - name: limit\n          in: query\n          type: integer\n          default: 10\n\
                   \x20     responses:\n        '200':\n          description: OK\n";

    let open_api = read_contents(v2_yaml);
    assert!(open_api.is_some());
    let open_api = open_api.unwrap();

    let params = get_parameters(&open_api, "/test");
    assert_eq!(params.len(), 1);

    let schema = params[0].get_schema();
    assert!(schema.is_some());
    let schema = schema.unwrap();
    assert_eq!(schema.get_default(), Some(JavaObject::Integer(10)), "Default should be 10");
    assert!(schema.get_default_set_flag(), "Default should be set");
}

#[test]
fn test_v2_boolean_parameter_with_default() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let v2_yaml = "swagger: '2.0'\n\
                   info:\n  title: Test\n  version: 1.0.0\n\
                   paths:\n  /test:\n    get:\n      parameters:\n        - name: verbose\n          in: query\n          type: boolean\n          default: false\n\
                   \x20     responses:\n        '200':\n          description: OK\n";

    let open_api = read_contents(v2_yaml);
    assert!(open_api.is_some());
    let open_api = open_api.unwrap();

    let params = get_parameters(&open_api, "/test");
    assert_eq!(params.len(), 1);

    let schema = params[0].get_schema();
    assert!(schema.is_some());
    let schema = schema.unwrap();
    assert_eq!(schema.get_default(), Some(JavaObject::Boolean(false)), "Default should be false");
    assert!(schema.get_default_set_flag(), "Default should be set");
}
