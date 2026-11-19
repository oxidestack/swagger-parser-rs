//! Port of `io.swagger.v3.parser.test.Issue1961PhantomBackRefTest`.

mod common;

use swagger_parser::core::ParseOptions;
use swagger_parser::parser::OpenAPIV3Parser;

const ROOT_DOCUMENT: &str = "src/test/resources/issue-1961-phantom/root.yaml";

/// A back-reference to a schema absent from the on-disk root document must surface an error, not bind to a
/// schema promoted during resolution
#[test]
fn back_ref_to_schema_missing_from_root_document_is_reported() {
    let _g = common::setup(common::V3_MODULE);
    let mut options = ParseOptions::new();
    options.resolve = true;
    let result = OpenAPIV3Parser::new().read_location(ROOT_DOCUMENT, None, Some(&options));

    assert!(result.openapi.is_some());
    let messages = result.messages.clone().unwrap_or_default();
    assert!(messages.iter().any(|m| m.contains("Could not find components/schemas/Payload")), "The invalid back-reference must be reported; got: {}", common::java_list(&messages));

    let schemas = result.openapi.unwrap().get_components().unwrap().get_schemas().unwrap();
    let payload = schemas.get("Payload");
    assert!(payload.is_some(), "The promoted external Payload schema itself is expected");

    let x = payload.unwrap().get_properties().unwrap().get("x").unwrap();
    assert_ne!(x.get_ref().as_deref(), Some("#/components/schemas/Payload"), "The phantom back-reference must not be silently rewritten into a self-reference to the promoted copy");
}
