//! Port of `io.swagger.v3.parser.test.OpenAPIV31ParserFullFlattenTest` (the `@BeforeClass` WireMock server → a
//! local stub server per test on a free port; the test itself reads a class path resource).

mod common;

use common::wiremock::{stub_files, WireMockServer};
use swagger_parser::core::ParseOptions;
use swagger_parser::parser::OpenAPIV3Parser;

const STUBS: &[(&str, &str, &str)] = &[
    ("src/test/resources/3.1.0/dereference/full/root.json", "/full/root.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/full/nested/domain.yaml", "/full/nested/domain.yaml", "application/yaml"),
    ("src/test/resources/3.1.0/dereference/full/ex.json", "/full/ex.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/full/ex1.json", "/full/ex1.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/full/ex2.json", "/full/ex2.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/full/ex1a.json", "/full/ex1a.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/full/nested/ex2a.json", "/full/nested/ex2a.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/full/nested/ex3a.json", "/full/nested/ex3a.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/full/ex1schema.json", "/full/ex1schema.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/full/ex2schema.json", "/full/ex2schema.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/full/ex3schema.json", "/full/ex3schema.json", "application/json"),
];

/// `@BeforeClass setUpWireMockServer`
fn set_up() -> WireMockServer {
    let server = WireMockServer::start(0, false);
    stub_files(&server, STUBS);
    server
}

#[test]
fn test_full() {
    let _g = common::setup(common::V3_MODULE);
    let _server = set_up();
    let p = ParseOptions { resolve: true, flatten: true, ..Default::default() };
    let open_api = OpenAPIV3Parser::new().read_with(Some("flatten31.json"), None, Some(&p));
    let open_api = open_api.expect("openAPI");
    assert!(open_api.get_components().unwrap().get_schemas().unwrap().get("ReturnInformation_manufacturer_signin_credentials").unwrap().get_required().is_some());
}
