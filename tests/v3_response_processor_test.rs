//! Port of `io.swagger.v3.parser.processors.ResponseProcessorTest` (`@Mocked SchemaProcessor`,
//! `HeaderProcessor`, `LinkProcessor`; injectable cache, OpenAPI, schema and header). Upstream the method's
//! `@Test` annotation is commented out; it is ported and run here.

mod common;

use common::mock_cache::{dyn_cache, MockCache};
use common::processor_mocks::{Mocked, ProcCall, Results};
use swagger_parser::models::*;
use swagger_parser::parser::processors::ResponseProcessor;

#[test]
fn test_process_response() {
    let _g = common::setup(common::V3_MODULE);
    let (cache, swagger) = (MockCache::new(), OpenAPI::new());
    let response_schema = Schema::new();
    let response_header = Header::new();
    let m = Mocked::new(&["SchemaProcessor", "HeaderProcessor", "LinkProcessor"], Results::default());

    let response = ApiResponse::new();
    response.content(Content::new().add_media_type("*/*", MediaType::new().schema(response_schema.clone())));
    response.add_header_object("foo", response_header.clone());

    ResponseProcessor::new31(dyn_cache(&cache), &swagger, false).process_response(&response).unwrap();

    assert_eq!(m.constructed("SchemaProcessor"), 1);
    assert_eq!(m.constructed("HeaderProcessor"), 1);
    assert_eq!(m.constructed("LinkProcessor"), 1);
    // FullVerifications: processSchema(responseSchema) and processHeader(responseHeader), once each
    let inv = m.invocations();
    assert_eq!(inv.len(), 2, "{inv:?}");
    assert_eq!(m.schema_count(&response_schema), 1);
    assert_eq!(inv.iter().filter(|c| matches!(c, ProcCall::Header(h) if h.ptr_eq(&response_header))).count(), 1);
}
