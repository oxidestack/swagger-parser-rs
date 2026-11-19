//! Port of `io.swagger.v3.parser.test.AnchorTest`.

mod common;

use jackson::JavaObject;
use swagger_parser::core::ParseOptions;
use swagger_parser::models::{json_pretty, OpenAPI};
use swagger_parser::parser::deserialization_utils::update_options;
use swagger_parser::parser::OpenAPIV3Parser;

/// `Json.pretty(openAPI)` (a null model serializes as `null`)
fn json_pretty_nullable(o: Option<&OpenAPI>) -> String {
    match o {
        None => "null".to_string(),
        Some(o) => json_pretty(o).unwrap(),
    }
}

#[test]
fn test_issue146() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = "openapi: 3.0.0\nservers: []\ninfo:\n  version: 0.0.1\n  title: API\nx-types:\n  OperationType: &OperationType\n    - registration\n# Describe your paths here\npaths:\n  /checker:\n    get:\n      parameters:\n        - name: operations\n          in: query\n          schema:\n            type: array\n            items:\n              type: string\n            enum: *OperationType\n            default:\n              - registration\n      responses:\n        '200':\n          description: OK\n          content:\n            '*/*':\n              schema:\n                $ref: \"#/components/schemas/OperationType\"\ncomponents:\n  schemas:\n    OperationType:\n      type: string\n      enum: *OperationType";
    let result = OpenAPIV3Parser::new().read_contents(yaml, None, None);
    let model = result.openapi.unwrap().get_components().unwrap().get_schemas().unwrap().get("OperationType").unwrap();
    assert_eq!(model.get_enum().unwrap().to_vec(), vec![JavaObject::string("registration")]);
}

#[test]
fn test_cycle() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = "a:\n  a1: &a1\n    a2: \n    - a3\n    - a4\n    a5: \n    - *a1";
    let result = OpenAPIV3Parser::new().read_contents(yaml, None, None);
    assert_eq!(json_pretty_nullable(result.openapi.as_ref()), "null");
}

#[test]
fn test_issue998() {
    let _g = common::setup(common::V3_MODULE);
    let result = OpenAPIV3Parser::new().read_location("./issue_998.yaml", None, None);
    assert!(result.openapi.is_none());
}

#[test]
fn test_issue998_billion() {
    let _g = common::setup(common::V3_MODULE);
    update_options(|o| o.max_yaml_references = 100000);
    let yaml = "a: &a [\"lol\",\"lol\",\"lol\",\"lol\",\"lol\",\"lol\",\"lol\",\"lol\",\"lol\"]\nb: &b [*a,*a,*a,*a,*a,*a,*a,*a,*a]\nc: &c [*b,*b,*b,*b,*b,*b,*b,*b,*b]\nd: &d [*c,*c,*c,*c,*c,*c,*c,*c,*c]\ne: &e [*d,*d,*d,*d,*d,*d,*d,*d,*d]\nf: &f [*e,*e,*e,*e,*e,*e,*e,*e,*e]\ng: &g [*f,*f,*f,*f,*f,*f,*f,*f,*f]\nh: &h [*g,*g,*g,*g,*g,*g,*g,*g,*g]\ni: &i [*h,*h,*h,*h,*h,*h,*h,*h,*h]";
    let yaml2 = "a: &a [\"lol\",\"lol\",\"lol\",\"lol\",\"lol\",\"lol\",\"lol\",\"lol\",\"lol\"]\nb: &b [*a,*a,*a,*a,*a,*a,*a,*a,*a]\nc: &c [*b,*b,*b,*b,*b,*b,*b,*b,*b]\nd: &d [*c,*c,*c,*c,*c,*c,*c,*c,*c]\ne: &e [*d,*d,*d,*d,*d,*d,*d,*d,*d]\nf: &f [*e,*e,*e,*e,*e,*e,*e,*e,*e]\ng: &g [*f,*f,*f,*f,*f,*f,*f,*f,*f]\nh: &h [*g,*g,*g,*g,*g,*g,*g,*g,*g]";
    let _yaml3 = "a: &a [\"lol\"]\nb: &b [*a,*a]\nc: &c [*b,*b]";
    let result = OpenAPIV3Parser::new().read_contents(yaml, None, None).openapi;
    assert_eq!(json_pretty_nullable(result.as_ref()), "null");
    let result = OpenAPIV3Parser::new().read_contents(yaml2, None, None).openapi;
    assert_eq!(json_pretty_nullable(result.as_ref()), "null");
    update_options(|o| o.max_yaml_references = 10000000);
}

#[test]
fn test_billion_laugh_protection_snake_yaml() {
    let _g = common::setup(common::V3_MODULE);
    let opts = ParseOptions { resolve: true, ..Default::default() };
    let result = OpenAPIV3Parser::new().read_location("billion_laughs_snake_yaml.yaml", None, Some(&opts));
    let schemas = result.openapi.unwrap().get_components().unwrap().get_schemas().unwrap();
    assert!(schemas.get("a1").is_some());
    assert_eq!(schemas.get("a1").unwrap().get_enum().unwrap().get(0), Some(JavaObject::string("AA1")));
    assert!(schemas.get("c1").is_some());
    assert_eq!(schemas.get("c1").unwrap().get_properties().unwrap().get("a").unwrap().get_enum().unwrap().get(0), Some(JavaObject::string("AA1")));

    update_options(|o| {
        o.max_yaml_aliases_for_collections = 50;
        o.yaml_allow_recursive_keys = false;
    });
    let _result = OpenAPIV3Parser::new().read_location("billion_laughs_snake_yaml.yaml", None, Some(&opts));
    update_options(|o| {
        o.max_yaml_aliases_for_collections = i32::MAX as usize;
        o.yaml_allow_recursive_keys = true;
    });
}
