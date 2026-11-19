//! Port of `io.swagger.v3.parser.test.ResponseResolverTest`.

mod common;

use swagger_parser::parser::OpenAPIV3Parser;

const SPEC: &str = "openapi: '3.0.0'\ninfo:\n  version: \"0.0.0\"\n  title: a test\npaths:\n  /persons:\n    get:\n      parameters: []\n      responses:\n        x-it-hurts: true\n        200:\n          description: Successful response\n";

#[test]
fn test_issue211() {
    let _g = common::setup(common::V3_MODULE);
    let result = OpenAPIV3Parser::new().read_contents(SPEC, None, None);
    let open_api = result.openapi.expect("openAPI");
    assert!(open_api.get_paths().unwrap().get("/persons").unwrap().get_get().unwrap().get_responses().unwrap().get("200").is_some());
}
