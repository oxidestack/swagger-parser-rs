//! Port of `io.swagger.v3.parser.test.Issue2222Test`.

mod common;

use swagger_parser::core::ParseOptions;
use swagger_parser::parser::OpenAPIV3Parser;

#[test]
fn should_map_same_entities_to_different_discriminator_values() {
    let _g = common::setup(common::V3_MODULE);
    let mut options = ParseOptions::new();
    options.resolve_fully = true;
    let result = OpenAPIV3Parser::new().read_location("issue-2222/openapi.yaml", None, Some(&options));

    // No error/warning messages should be present, especially Duplicate key ones
    assert_eq!(0, result.messages.as_ref().unwrap().len(), "{:?}", result.messages);
    let open_api = result.openapi.unwrap();
    assert_eq!(1, open_api.get_paths().unwrap().len());
    let response = open_api.get_paths().unwrap().get("/mypath").unwrap().read_operations()[0].get_responses().unwrap().get("200").unwrap();
    let discriminator = response.get_content().unwrap().get("application/json").unwrap().get_schema().unwrap().get_items().unwrap().get_discriminator().unwrap();
    assert_eq!(Some("discriminatorProperty".to_string()), discriminator.get_property_name());
    let mapping = discriminator.get_mapping().unwrap();
    assert_eq!(Some("#/components/schemas/TypeA".to_string()), mapping.get("ONE"));
    assert_eq!(Some("#/components/schemas/TypeB".to_string()), mapping.get("TWO"));
    assert_eq!(Some("#/components/schemas/TypeB".to_string()), mapping.get("THREE"));
    assert_eq!(Some("#/components/schemas/TypeB".to_string()), mapping.get("FOUR"));
    assert_eq!(Some("#/components/schemas/TypeB".to_string()), mapping.get("FIVE"));
}
