//! Port of `io.swagger.v3.parser.test.OpenAPIV31ParserSchemaIndirectTest` (the `@BeforeClass` WireMock server → a local stub server per
//! test on a free port).

mod common;

use common::wiremock::{stub_files, WireMockServer};
use swagger_parser::core::ParseOptions;
use swagger_parser::models::yaml31_pretty;
use swagger_parser::parser::OpenAPIV3Parser;

const STUBS: &[(&str, &str, &str)] = &[
    ("src/test/resources/3.1.0/dereference/schemaindirect/root.json", "/schemaindirect/root.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/schemaindirect/nested/domain.yaml", "/schemaindirect/nested/domain.yaml", "application/yaml"),
    ("src/test/resources/3.1.0/dereference/schemaindirect/ex.json", "/schemaindirect/ex.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/schemaindirect/ex1.json", "/schemaindirect/ex1.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/schemaindirect/ex2.json", "/schemaindirect/ex2.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/schemaindirect/ex1a.json", "/schemaindirect/ex1a.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/schemaindirect/nested/ex2a.json", "/schemaindirect/nested/ex2a.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/schemaindirect/nested/ex3a.json", "/schemaindirect/nested/ex3a.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/schemaindirect/ex1schema.json", "/schemaindirect/ex1schema.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/schemaindirect/ex2schema.json", "/schemaindirect/ex2schema.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/schemaindirect/ex3schema.json", "/schemaindirect/ex3schema.json", "application/json"),
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
    let server = set_up();
    let p = ParseOptions { resolve: true, ..Default::default() };
    let uri = format!("http://localhost:{}/schemaindirect/root.json", server.port());
    let result = OpenAPIV3Parser::new().read_location(&uri, None, Some(&p));
    assert_eq!(yaml31_pretty(&result.openapi.unwrap()).unwrap(), common::read_file("src/test/resources/3.1.0/dereference/schemaindirect/dereferenced.yaml"));
}
