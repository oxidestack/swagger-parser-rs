//! Port of `io.swagger.v3.parser.test.OpenAPIV31ParserPathItemTest` (the `@BeforeClass` WireMock server → a local stub server per
//! test on a free port).

mod common;

use common::wiremock::{stub_files, WireMockServer};
use swagger_parser::core::ParseOptions;
use swagger_parser::models::yaml31_pretty;
use swagger_parser::parser::OpenAPIV3Parser;

const STUBS: &[(&str, &str, &str)] = &[
    ("src/test/resources/3.1.0/dereference/pathItem/basic/root.yaml", "/basic/root.yaml", "application/yaml"),
    ("src/test/resources/3.1.0/dereference/pathItem/basic/nested/domain.yaml", "/basic/nested/domain.yaml", "application/yaml"),
    ("src/test/resources/3.1.0/dereference/pathItem/external-only/root.json", "/external-only/root.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/pathItem/external-only/ex.json", "/external-only/ex.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/pathItem/external-indirections/root.json", "/external-indirections/root.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/pathItem/external-indirections/ex1.json", "/external-indirections/ex1.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/pathItem/external-indirections/ex2.json", "/external-indirections/ex2.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/pathItem/direct-external-circular/root.json", "/direct-external-circular/root.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/pathItem/direct-external-circular/ex.json", "/direct-external-circular/ex.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/pathItem/indirect-external-circular/root.json", "/indirect-external-circular/root.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/pathItem/indirect-external-circular/ex1.json", "/indirect-external-circular/ex1.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/pathItem/indirect-external-circular/ex2.json", "/indirect-external-circular/ex2.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/pathItem/internal-external/root.json", "/internal-external/root.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/pathItem/internal-external/ex.json", "/internal-external/ex.json", "application/json"),
    ("src/test/resources/3.1.0/dereference/pathItem/external-internal-nested/root.yaml", "/external-internal-nested/root.yaml", "application/yaml"),
    ("src/test/resources/3.1.0/dereference/pathItem/external-internal-nested/nested/domain.yaml", "/external-internal-nested/nested/domain.yaml", "application/yaml"),
];

/// `@BeforeClass setUpWireMockServer`
fn set_up() -> WireMockServer {
    let server = WireMockServer::start(0, false);
    stub_files(&server, STUBS);
    server
}

#[test]
fn test_path_item_basic_ref() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up();
    let p = ParseOptions { resolve: true, ..Default::default() };
    let uri = format!("http://localhost:{}/basic/root.yaml", server.port());
    let result = OpenAPIV3Parser::new().read_location(&uri, None, Some(&p));
    assert!(result.openapi.is_some());
    assert!(result.messages.as_ref().unwrap().is_empty());
    assert_eq!(yaml31_pretty(&result.openapi.unwrap()).unwrap(), "openapi: 3.1.0\ninfo:\n  title: parse-api\n  description: Test swagger-parser\n  version: 1.0.0\nservers:\n- url: /\npaths:\n  /externalref:\n    get:\n      description: ExternalRef PathItem\n      operationId: ExternalRef PathItem\n      responses:\n        \"200\":\n          description: OK\n  /relativeref:\n    get:\n      description: RelativeRef PathItem\n      operationId: RelativeRef PathItem\n      responses:\n        \"200\":\n          description: OK\n  /internalref:\n    $ref: \"#/components/pathItems/InternalRef\"\n  /internalreftoexternal:\n    $ref: \"#/components/pathItems/InternalRefToExternal\"\n  /internal:\n    get:\n      description: Internal PathItem\n      operationId: Internal PathItem\n      responses:\n        \"200\":\n          description: OK\ncomponents:\n  pathItems:\n    InternalRefToExternal:\n      get:\n        description: DomainInternalRefToExternal PathItem\n        operationId: DomainInternalRefToExternal PathItem\n        responses:\n          \"200\":\n            description: OK\n    InternalRef:\n      get:\n        description: InternalRef PathItem\n        operationId: InternalRef PathItem\n        responses:\n          \"200\":\n            description: OK\n");
}

#[test]
fn test_path_item_external_internal_nested_ref() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up();
    let p = ParseOptions { resolve: true, ..Default::default() };
    let uri = format!("http://localhost:{}/external-internal-nested/root.yaml", server.port());
    let result = OpenAPIV3Parser::new().read_location(&uri, None, Some(&p));
    assert!(result.openapi.is_some());
    assert!(result.messages.as_ref().unwrap().is_empty());
    assert_eq!(yaml31_pretty(&result.openapi.unwrap()).unwrap(), "openapi: 3.1.0\ninfo:\n  title: parse-api\n  description: Test swagger-parser\n  version: 1.0.0\nservers:\n- url: /\npaths:\n  /externalref:\n    get:\n      description: InternalDomainRef PathItem\n      operationId: InternalDomainRef PathItem\n      responses:\n        \"200\":\n          description: OK\n  /relativeref:\n    get:\n      description: RelativeRef PathItem\n      operationId: RelativeRef PathItem\n      responses:\n        \"200\":\n          description: OK\n  /internalref:\n    $ref: \"#/components/pathItems/InternalRef\"\n  /internal:\n    get:\n      description: Internal PathItem\n      operationId: Internal PathItem\n      responses:\n        \"200\":\n          description: OK\ncomponents:\n  pathItems:\n    InternalRef:\n      get:\n        description: InternalRef PathItem\n        operationId: InternalRef PathItem\n        responses:\n          \"200\":\n            description: OK\n");
}

#[test]
fn test_path_item_external_only_ref() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up();
    let p = ParseOptions { resolve: true, ..Default::default() };
    let uri = format!("http://localhost:{}/external-only/root.json", server.port());
    let result = OpenAPIV3Parser::new().read_location(&uri, None, Some(&p));
    assert!(result.openapi.is_some());
    assert!(!result.messages.as_ref().unwrap().is_empty());
    assert_eq!(yaml31_pretty(&result.openapi.unwrap()).unwrap(), "openapi: 3.1.0\nservers:\n- url: /\npaths:\n  /path1:\n    summary: path item summary\n    description: path item description\n    get: {}\n");
}

#[test]
fn test_path_item_external_indirections_ref() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up();
    let p = ParseOptions { resolve: true, ..Default::default() };
    let uri = format!("http://localhost:{}/external-indirections/root.json", server.port());
    let result = OpenAPIV3Parser::new().read_location(&uri, None, Some(&p));
    assert!(result.openapi.is_some());
    assert!(!result.messages.as_ref().unwrap().is_empty());
    assert_eq!(yaml31_pretty(&result.openapi.unwrap()).unwrap(), "openapi: 3.1.0\nservers:\n- url: /\npaths:\n  /path1:\n    summary: path item summary\n    description: path item description\n    get: {}\n");
}

#[test]
fn test_path_item_direct_external_circular_ref() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up();
    let p = ParseOptions { resolve: true, ..Default::default() };
    let uri = format!("http://localhost:{}/direct-external-circular/root.json", server.port());
    let result = OpenAPIV3Parser::new().read_location(&uri, None, Some(&p));
    assert!(result.openapi.is_some());
    assert!(!result.messages.as_ref().unwrap().is_empty());
    assert_eq!(yaml31_pretty(&result.openapi.unwrap()).unwrap(), "openapi: 3.1.0\nservers:\n- url: /\npaths:\n  /path1:\n    $ref: ./ex.json\n");
}

#[test]
fn test_path_item_indirect_external_circular_ref() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up();
    let p = ParseOptions { resolve: true, ..Default::default() };
    let uri = format!("http://localhost:{}/indirect-external-circular/root.json", server.port());
    let result = OpenAPIV3Parser::new().read_location(&uri, None, Some(&p));
    assert!(result.openapi.is_some());
    assert!(!result.messages.as_ref().unwrap().is_empty());
    assert_eq!(yaml31_pretty(&result.openapi.unwrap()).unwrap(), "openapi: 3.1.0\nservers:\n- url: /\npaths:\n  /path1:\n    $ref: ./ex1.json\n");
}

#[test]
fn test_path_item_internal_external_ref() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up();
    let p = ParseOptions { resolve: true, ..Default::default() };
    let uri = format!("http://localhost:{}/internal-external/root.json", server.port());
    let result = OpenAPIV3Parser::new().read_location(&uri, None, Some(&p));
    assert!(result.openapi.is_some());
    assert!(!result.messages.as_ref().unwrap().is_empty());
    assert_eq!(yaml31_pretty(&result.openapi.unwrap()).unwrap(), "openapi: 3.1.0\nservers:\n- url: /\npaths:\n  /path1:\n    summary: path item summary\n    description: path item description\n    get: {}\n  /path3:\n    summary: path item summary\n    description: path item description\n    get: {}\n  /path4:\n    summary: path item summary\n    description: path item description\n    get: {}\n");
}

