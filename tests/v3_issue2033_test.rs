//! Port of `io.swagger.v3.parser.test.Issue2033Test`.

mod common;

use jackson::JavaObject;
use swagger_parser::core::ParseOptions;
use swagger_parser::parser::OpenAPIV3Parser;

#[test]
fn resolves_relative_component_refs_from_external_templated_path_item() {
    let _g = common::setup(common::V3_MODULE);
    let mut options = ParseOptions::new();
    options.resolve = true;
    let result = OpenAPIV3Parser::new().read_location("src/test/resources/issue-2033/main.yaml", None, Some(&options));

    assert!(result.openapi.is_some());
    let messages = result.messages.clone().unwrap_or_default();
    assert!(messages.is_empty(), "Unexpected parser messages: {}", common::java_list(&messages));

    let open_api = result.openapi.unwrap();
    let partial_response = open_api.get_paths().unwrap().get("/nodes/{uuid}/rights").unwrap().get_get().unwrap().get_responses().unwrap().get("206").unwrap();

    let content_range = partial_response.get_headers().unwrap().get("Content-Range").expect("Content-Range");
    assert_eq!(content_range.get_ref().as_deref(), Some("#/components/headers/Content-Range"));

    let resolved_content_range = open_api.get_components().unwrap().get_headers().unwrap().get("Content-Range").expect("resolved Content-Range");
    let s = resolved_content_range.get_schema().expect("schema");
    assert_eq!(s.get_type().as_deref(), Some("string"));
    assert_eq!(s.get_pattern().as_deref(), Some("\\d+-\\d+/\\d+"));

    let next_page = partial_response.get_links().unwrap().get("nextPage").expect("nextPage");
    assert_eq!(next_page.get_ref().as_deref(), Some("#/components/links/NextPage"));

    let resolved_next_page = open_api.get_components().unwrap().get_links().unwrap().get("NextPage").expect("NextPage");
    assert_eq!(resolved_next_page.get_operation_id().as_deref(), Some("listNodeRights"));

    let content_range_example = partial_response.get_content().unwrap().get("application/json").unwrap().get_examples().unwrap().get("contentRange").expect("contentRange");
    assert_eq!(content_range_example.get_ref().as_deref(), Some("#/components/examples/ContentRangeExample"));

    let resolved_example = open_api.get_components().unwrap().get_examples().unwrap().get("ContentRangeExample").expect("ContentRangeExample");
    assert_eq!(resolved_example.get_value(), Some(JavaObject::string("items 0-9/42")));
}
