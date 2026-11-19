//! Port of `io.swagger.v3.parser.test.OpenAPIV31ParserExampleTest` (the `@BeforeClass` WireMock server → a local stub server per
//! test on a free port).

mod common;

use common::wiremock::{stub_files, WireMockServer};
use swagger_parser::core::ParseOptions;
use swagger_parser::models::yaml31_pretty;
use swagger_parser::parser::OpenAPIV3Parser;

const STUBS: &[(&str, &str, &str)] = &[
    ("src/test/resources/3.1.0/dereference/examples/root.json", "/examples/root.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/examples/nested/domain.yaml", "/examples/nested/domain.yaml", "application/yaml"),
    ("src/test/resources/3.1.0/dereference/examples/ex.json", "/examples/ex.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/examples/ex1.json", "/examples/ex1.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/examples/ex2.json", "/examples/ex2.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/examples/ex1a.json", "/examples/ex1a.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/examples/nested/ex2a.json", "/examples/nested/ex2a.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/examples/nested/ex3a.json", "/examples/nested/ex3a.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/examples/ex1schema.json", "/examples/ex1schema.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/examples/ex2schema.json", "/examples/ex2schema.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/examples/ex3schema.json", "/examples/ex3schema.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/examples/example.json", "/examples/example.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/examples/param.json", "/examples/param.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/examples/exampleindirect.json", "/examples/exampleindirect.json", "application/json"),
];

/// `@BeforeClass setUpWireMockServer`
fn set_up() -> WireMockServer {
    let server = WireMockServer::start(0, false);
    stub_files(&server, STUBS);
    server
}

#[test]
fn test_examples() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up();
    let p = ParseOptions { resolve: true, ..Default::default() };
    let uri = format!("http://localhost:{}/examples/root.json", server.port());
    let result = OpenAPIV3Parser::new().read_location(&uri, None, Some(&p));
    assert_eq!(yaml31_pretty(&result.openapi.unwrap()).unwrap(), common::read_file("src/test/resources/3.1.0/dereference/examples/dereferenced.yaml"));
}
