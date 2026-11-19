//! Port of `io.swagger.v3.parser.test.OpenAPIV3ParserRemoteResolvingTest` (the `@BeforeClass` WireMock server →
//! a local stub server per test on a free port).

mod common;

use common::wiremock::{a_response, get, url_path_matching, WireMockServer};
use swagger_parser::core::{AuthorizationValue, ParseOptions};
use swagger_parser::parser::OpenAPIV3Parser;

/// (file, url path regex, content type, replace `${dynamicPort}`)
const STUBS: &[(&str, &str, &str, bool)] = &[
    ("src/test/resources/remote_references/remote_pathItem.yaml.template", "/remote/path", "application/yaml", false),
    ("src/test/resources/remote_references/remote_schema.yaml", "/remote/schema", "application/yaml", false),
    ("src/test/resources/remote_references/remote_responses.yaml.template", "/remote/response", "application/yaml", true),
    ("src/test/resources/remote_references/remote_requestBody.yaml", "/remote/requestBody", "application/yaml", false),
    ("src/test/resources/remote_references/remote_parameter.yaml.template", "/remote/parameter", "application/yaml", true),
    ("src/test/resources/remote_references/remote_example.yaml", "/remote/example", "application/yaml", false),
    ("src/test/resources/remote_references/remote_link.yaml", "/remote/link", "application/yaml", false),
    ("src/test/resources/remote_references/remote_callback.yaml", "/remote/callback", "application/yaml", false),
    ("src/test/resources/remote_references/remote_securityScheme.yaml", "/remote/security", "application/yaml", false),
    ("src/test/resources/oas4.yaml", "/remote/spec", "application/yaml", false),
    ("src/test/resources/flatten.json", "/remote/json", "application/json", false),
];

fn stub(server: &WireMockServer, url: &str, ct: &str, body: String) {
    server.stub_for(get(url_path_matching(url)).will_return(a_response().with_status(200).with_header("Content-type", ct).with_body(body)));
}

/// `@BeforeClass setUpWireMockServer`
fn set_up() -> WireMockServer {
    let server = WireMockServer::start(0, false);
    for (file, url, ct, replace) in STUBS {
        let mut body = common::read_file(file);
        if *replace {
            body = body.replace("${dynamicPort}", &server.port().to_string());
        }
        stub(&server, url, ct, body);
    }
    server
}

/// `List<AuthorizationValue> auths = new ArrayList<>()`
fn auths() -> Vec<AuthorizationValue> {
    Vec::new()
}

fn template(server: &WireMockServer, file: &str) -> String {
    common::read_file(file).replace("${dynamicPort}", &server.port().to_string())
}

#[test]
fn test30() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up();
    let path_file = template(&server, "src/test/resources/oas3.yaml.template");
    let options = ParseOptions { resolve: true, ..Default::default() };
    let result = OpenAPIV3Parser::new().read_contents(&path_file, Some(&auths()), Some(&options));
    let open_api = result.openapi.unwrap();
    assert_eq!(open_api.get_openapi().as_deref(), Some("3.0.1"));
    assert_eq!(open_api.get_components().unwrap().get_schemas().unwrap().get("OrderRef").unwrap().get_type().as_deref(), Some("object"));
}

#[test]
fn test_resolve_fully() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up();
    let path_file = template(&server, "src/test/resources/oas3.yaml.template");
    let options = ParseOptions { resolve_fully: true, ..Default::default() };
    let result = OpenAPIV3Parser::new().read_contents(&path_file, Some(&[]), Some(&options));
    let open_api = result.openapi.unwrap();
    assert_eq!(open_api.get_openapi().as_deref(), Some("3.0.1"));
    assert_eq!(open_api.get_components().unwrap().get_schemas().unwrap().get("OrderRef").unwrap().get_type().as_deref(), Some("object"));
}

#[test]
fn issue1455_test_resolve_fully_v2_should_not_throw_npe() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up();
    let path_file = template(&server, "src/test/resources/swagger.json");
    let options = ParseOptions { resolve_fully: true, ..Default::default() };
    let result = OpenAPIV3Parser::new().read_contents(&path_file, Some(&[]), Some(&options));
    assert!(result.openapi.is_none());
    let messages = result.messages.expect("messages");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0], "attribute openapi is missing");
}

#[test]
fn test_resolve_fully_example() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up();
    let path_file = template(&server, "src/test/resources/oas3.yaml.template");
    let options = ParseOptions { resolve_fully: true, ..Default::default() };
    let result = OpenAPIV3Parser::new().read_contents(&path_file, Some(&[]), Some(&options));
    let open_api = result.openapi.unwrap();
    let components = open_api.get_components().unwrap();
    let response = open_api.get_paths().unwrap().get("/mockResponses/objectMultipleExamples").unwrap().get_get().unwrap().get_responses().unwrap().get("200").unwrap();
    let content = response.get_content().unwrap();
    assert_eq!(content.get("application/json").unwrap().get_examples().unwrap().get("ArthurDent"), components.get_examples().unwrap().get("Arthur"));
    assert_eq!(content.get("application/xml").unwrap().get_examples().unwrap().get("Trillian"), components.get_examples().unwrap().get("Trillian"));
}

fn assert_flattened_user(open_api: &swagger_parser::models::OpenAPI) {
    let schemas = open_api.get_components().unwrap().get_schemas().unwrap();
    let user = schemas.get("User").expect("User");
    let address = user.get_properties().unwrap().get("address").unwrap();
    assert!(address.get_ref().is_some());
    let user_address = schemas.get("User_address").expect("User_address");
    assert!(user_address.get_properties().unwrap().get("city").is_some());
    assert!(user_address.get_properties().unwrap().get("street").is_some());
}

#[test]
fn test_inline_model_resolver() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up();
    let path_file = template(&server, "src/test/resources/flatten.json");
    let options = ParseOptions { flatten: true, ..Default::default() };
    let result = OpenAPIV3Parser::new().read_contents(&path_file, Some(&auths()), Some(&options));
    assert_flattened_user(&result.openapi.expect("openAPI"));
}

#[test]
fn test30_no_options() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up();
    let path_file = template(&server, "src/test/resources/oas3.yaml.template");
    let result = OpenAPIV3Parser::new().read_contents(&path_file, Some(&auths()), None);
    let open_api = result.openapi.unwrap();
    assert_eq!(open_api.get_openapi().as_deref(), Some("3.0.1"));
    assert_eq!(open_api.get_components().unwrap().get_schemas().unwrap().get("OrderRef").unwrap().get_type().as_deref(), Some("object"));
}

#[test]
fn test_shell_method() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up();
    let url = "http://localhost:${dynamicPort}/remote/spec".replace("${dynamicPort}", &server.port().to_string());
    let open_api = OpenAPIV3Parser::new().read(&url).expect("openAPI");
    assert_eq!(open_api.get_openapi().as_deref(), Some("3.0.1"));
}

#[test]
fn test_inline_model_resolver_by_url() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up();
    let url = "http://localhost:${dynamicPort}/remote/json".replace("${dynamicPort}", &server.port().to_string());
    let options = ParseOptions { flatten: true, ..Default::default() };
    let result = OpenAPIV3Parser::new().read_location(&url, Some(&[]), Some(&options));
    assert_flattened_user(&result.openapi.expect("openAPI"));
}

#[test]
fn test_issue_same_refs_different_model() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up();
    stub(&server, "/issue-domain/", "application/json", common::read_file("src/test/resources/same-refs-different-model-domain.yaml"));
    let path_file = template(&server, "src/test/resources/same-refs-different-model.yaml");
    let options = ParseOptions { resolve: true, resolve_fully: true, ..Default::default() };
    let open_api = OpenAPIV3Parser::new().read_contents(&path_file, None, Some(&options));
    // Yaml.prettyPrint(openAPI) only prints the result
    assert_eq!(open_api.messages.as_ref().unwrap().len(), 0, "{:?}", open_api.messages);
}

#[test]
fn test_issue251() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up();
    stub(&server, "/domain", "application/json", common::read_file("src/test/resources/domain.yaml"));
    let path_file = template(&server, "src/test/resources/issue251.yaml");
    let options = ParseOptions { resolve: true, ..Default::default() };
    let parse_result = OpenAPIV3Parser::new().read_contents(&path_file, None, Some(&options));
    assert_eq!(parse_result.messages.as_ref().unwrap().len(), 0, "{:?}", parse_result.messages);
    let open_api = parse_result.openapi.unwrap();
    assert_eq!(open_api.get_components().unwrap().get_schemas().unwrap().len(), 2);
    let param = open_api.get_paths().unwrap().get("/parse").unwrap().get_get().unwrap().get_parameters().unwrap().get(0).unwrap();
    assert_eq!(param.get_schema().unwrap().get_ref().as_deref(), Some("#/components/schemas/Parse"));
}
