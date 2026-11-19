//! Port of `io.swagger.v3.parser.test.OpenAPIV31ParserSchemaTest` (the `@BeforeClass` WireMock server → a local
//! stub server per test on a free port).

mod common;

use common::wiremock::{a_response, get, stub_files, url_path_matching, WireMockServer};
use swagger_parser::core::{ParseOptions, SwaggerParseResult};
use swagger_parser::models::{json_string_with, Mapper, MapperFeatures};
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
    // served as is (no ${dynamicPort} replacement)
    let path_file = common::read_file("src/test/resources/3.1.0/dereference/schema/external-oas-with-id-defs/oas.yaml");
    server.stub_for(get(url_path_matching("/oai-v3.1-schema.yaml")).will_return(a_response().with_status(200).with_header("Content-type", "application/yaml").with_body(path_file)));
    server
}

/// `new File(path).getAbsolutePath()`
fn absolute(path: &str) -> String {
    std::env::current_dir().unwrap().join(path).to_string_lossy().into_owned()
}

fn read_resolved(dir: &str) -> SwaggerParseResult {
    let p = ParseOptions { resolve: true, ..Default::default() };
    OpenAPIV3Parser::new().read_location(&absolute(&format!("src/test/resources/3.1.0/dereference/schema/{dir}/root.json")), None, Some(&p))
}

/// `Json31.mapper().copy()` with `ORDER_MAP_ENTRIES_BY_KEYS` and `SORT_PROPERTIES_ALPHABETICALLY`, written with a
/// `DefaultPrettyPrinter`, against `dereferenced.json`.
fn compare(dir: &str, result: &SwaggerParseResult) {
    let features = MapperFeatures { sort_properties_alphabetically: true, order_map_entries_by_keys: true };
    let actual = json_string_with(result.openapi.as_ref().unwrap(), Mapper::Json31, features, true).unwrap();
    assert_eq!(actual, common::read_file(&format!("src/test/resources/3.1.0/dereference/schema/{dir}/dereferenced.json")));
}

#[test]
fn test_id_url_external() {
    let _g = common::setup(common::V3_MODULE);
    let _server = set_up();
    compare("$id-uri-external", &read_resolved("$id-uri-external"));
}

#[test]
fn test_id_url_enclosing() {
    let _g = common::setup(common::V3_MODULE);
    let _server = set_up();
    compare("$id-uri-enclosing", &read_resolved("$id-uri-enclosing"));
}

#[test]
fn test_id_url_direct() {
    let _g = common::setup(common::V3_MODULE);
    let _server = set_up();
    compare("$id-uri-direct", &read_resolved("$id-uri-direct"));
}

#[test]
fn test_id_url_unresolvable() {
    let _g = common::setup(common::V3_MODULE);
    let _server = set_up();
    compare("$id-unresolvable", &read_resolved("$id-unresolvable"));
}

#[test]
fn test_anchor_ext() {
    let _g = common::setup(common::V3_MODULE);
    let _server = set_up();
    compare("$anchor-external", &read_resolved("$anchor-external"));
}

#[test]
fn test_anchor_int() {
    let _g = common::setup(common::V3_MODULE);
    let _server = set_up();
    compare("$anchor-internal", &read_resolved("$anchor-internal"));
}

#[test]
fn test_anchor_unresolve() {
    let _g = common::setup(common::V3_MODULE);
    let _server = set_up();
    compare("$anchor-not-found", &read_resolved("$anchor-not-found"));
}

#[test]
fn test_external_schema_with_id_and_defs_resolution_issue2271() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up();
    let root_yaml = common::read_file("src/test/resources/3.1.0/dereference/schema/external-oas-with-id-defs/root.yaml").replace("${dynamicPort}", &server.port().to_string());
    let options = ParseOptions { resolve: true, ..Default::default() };
    let result = OpenAPIV3Parser::new().read_contents(&root_yaml, None, Some(&options));
    assert!(result.openapi.is_some());
    assert!(
        result.messages.as_ref().is_none_or(|m| m.iter().all(|m| !m.contains("Could not find /$defs"))),
        "Expected no $defs resolution errors when resolving external schema with $id, but got: {:?}",
        result.messages
    );
}
