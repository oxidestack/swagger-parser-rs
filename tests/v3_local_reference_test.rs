//! Port of `io.swagger.v3.parser.test.LocalReferenceTest` (`@Mocked RemoteUrl`).

mod common;

use common::{mock_remote_url, read_file};
use swagger_parser::core::ParseOptions;
use swagger_parser::models::*;
use swagger_parser::parser::OpenAPIV3Parser;

#[test]
fn test_auth() {
    let _g = common::setup(common::V3_MODULE);
    let issue_454_yaml = read_file("src/test/resources/nested-network-references/issue-454.yaml");
    let issue_454_components_yaml = read_file("src/test/resources/nested-network-references/issue-454-components.yaml");
    let _remote = mock_remote_url(&[
        ("https://remote-server.com/issue-454.yaml", &issue_454_yaml),
        ("https://remote-components.com/issue-454-components", &issue_454_components_yaml),
    ]);
    let mut options = ParseOptions::new();
    options.resolve = true;
    let swagger = OpenAPIV3Parser::new().read_with(Some("https://remote-server.com/issue-454.yaml"), None, Some(&options)).expect("openAPI");

    let schemas = swagger.get_components().unwrap().get_schemas().unwrap();
    assert!(schemas.get("ErrorModel").is_some());
    assert!(schemas.get("ModelWithNestedProperties").is_some());

    let model = schemas.get("ModelWithNestedProperties").unwrap();
    let property = model.get_properties().unwrap().get("remoteProperty").expect("remoteProperty");
    assert!(property.get_ref().is_some());
    assert_eq!(property.get_ref().as_deref(), Some("#/components/schemas/RemoteComponent"));
    assert!(schemas.get("NestedProperty").is_some());

    let nested_model = schemas.get("NestedProperty").expect("NestedProperty");
    assert!(nested_model.get_properties().unwrap().get("name").is_some());
    // instanceof StringSchema
    assert_eq!(nested_model.get_properties().unwrap().get("name").unwrap().kind(), SchemaKind::String);
}
