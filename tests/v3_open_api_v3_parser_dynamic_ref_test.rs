//! Port of `io.swagger.v3.parser.test.OpenAPIV3ParserDynamicRefTest`.

mod common;

use swagger_parser::parser::OpenAPIV3Parser;

#[test]
fn test_dynamic_ref_parsing() {
    let _g = common::setup(common::V3_MODULE);
    let result = OpenAPIV3Parser::new().read_location("dynamicRef/dynamicref-example.yaml", None, None);
    let open_api = result.openapi.expect("Parsed OpenAPI object should not be null");
    let root_schema = open_api
        .get_paths()
        .unwrap()
        .get("/tree")
        .unwrap()
        .get_get()
        .unwrap()
        .get_responses()
        .unwrap()
        .get("200")
        .unwrap()
        .get_content()
        .unwrap()
        .get("application/json")
        .unwrap()
        .get_schema()
        .unwrap();
    assert_eq!(root_schema.get_ref().as_deref(), Some("#/components/schemas/Node"), "Expected root schema to be a $ref to Node");
    let node_schema = open_api.get_components().unwrap().get_schemas().unwrap().get("Node").expect("Node schema should be parsed");
    let children_schema = node_schema.get_properties().unwrap().get("children").unwrap();
    let items_schema = children_schema.get_items().unwrap();
    assert_eq!(items_schema.get_dynamic_ref().as_deref(), Some("#node"), "Expected $dynamicRef to be preserved");
}
