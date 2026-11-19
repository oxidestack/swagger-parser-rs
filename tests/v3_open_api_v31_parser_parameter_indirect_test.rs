//! Port of `io.swagger.v3.parser.test.OpenAPIV31ParserParameterIndirectTest` (the `@BeforeClass` WireMock server → a local stub server per
//! test on a free port).

mod common;

use common::wiremock::{stub_files, WireMockServer};
use swagger_parser::core::ParseOptions;
use swagger_parser::models::yaml31_pretty;
use swagger_parser::parser::OpenAPIV3Parser;

const STUBS: &[(&str, &str, &str)] = &[
    ("src/test/resources/3.1.0/dereference/parameterindirect/root.json", "/parameterindirect/root.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/parameterindirect/nested/domain.yaml", "/parameterindirect/nested/domain.yaml", "application/yaml"),
    ("src/test/resources/3.1.0/dereference/parameterindirect/ex.json", "/parameterindirect/ex.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/parameterindirect/ex1.json", "/parameterindirect/ex1.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/parameterindirect/ex2.json", "/parameterindirect/ex2.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/parameterindirect/ex1a.json", "/parameterindirect/ex1a.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/parameterindirect/nested/ex2a.json", "/parameterindirect/nested/ex2a.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/parameterindirect/nested/ex3a.json", "/parameterindirect/nested/ex3a.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/parameterindirect/ex1schema.json", "/parameterindirect/ex1schema.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/parameterindirect/ex2schema.json", "/parameterindirect/ex2schema.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/parameterindirect/ex3schema.json", "/parameterindirect/ex3schema.json", "application/json"),
];

/// `@BeforeClass setUpWireMockServer`
fn set_up() -> WireMockServer {
    let server = WireMockServer::start(0, false);
    stub_files(&server, STUBS);
    server
}

#[test]
fn test_parameter() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up();
    let p = ParseOptions { resolve: true, ..Default::default() };
    let uri = format!("http://localhost:{}/parameterindirect/root.json", server.port());
    let result = OpenAPIV3Parser::new().read_location(&uri, None, Some(&p));
    assert_eq!(yaml31_pretty(&result.openapi.unwrap()).unwrap(), common::read_file("src/test/resources/3.1.0/dereference/parameterindirect/dereferenced.yaml"));
}
