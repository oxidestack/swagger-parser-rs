//! Port of `io.swagger.v3.parser.test.RelativeReferenceTest` (`@Mocked RemoteUrl`).

mod common;

use common::mock_remote_url;
use std::collections::HashSet;
use swagger_parser::core::ParseOptions;
use swagger_parser::parser::OpenAPIV3Parser;

const SPEC: &str = "openapi: 3.0.0\nservers:\n  - url: /\n  - url: https://localhost:8080/{version}\n    description: The local server\n    variables:\n      version:\n        default: v2\n        enum:\n          - v1\n          - v2\ninfo:\n  description: It works.\n  version: 1.0.0\n  title: My API\npaths:\n  /samplePath:\n    $ref: './path/samplePath.yaml'";
const SAMPLE_PATH: &str = "get:\n  responses:\n    '200':\n      description: It works\n  requestBody:\n    content:\n      application/json:\n        schema:\n          type: object\n    required: true";

#[test]
fn test_issue_server_url_validation() {
    let _g = common::setup(common::V3_MODULE);
    let remote = mock_remote_url(&[("http://foo.bar.com/swagger.json", SPEC)]);

    let result = OpenAPIV3Parser::new().read_location("http://foo.bar.com/swagger.json", None, Some(&ParseOptions::new()));
    assert_eq!(remote.count("http://foo.bar.com/swagger.json"), 1);
    assert!(result.openapi.is_some());
    assert!(result.messages.as_ref().unwrap().is_empty(), "{:?}", result.messages);
}

#[test]
fn test_issue213() {
    let _g = common::setup(common::V3_MODULE);
    let remote = mock_remote_url(&[("http://foo.bar.com/swagger.json", SPEC), ("http://foo.bar.com/path/samplePath.yaml", SAMPLE_PATH)]);

    let swagger = OpenAPIV3Parser::new().read("http://foo.bar.com/swagger.json").expect("openAPI");
    assert_eq!(remote.count("http://foo.bar.com/swagger.json"), 1);
    assert_eq!(remote.count("http://foo.bar.com/path/samplePath.yaml"), 1);
    let sample = swagger.get_paths().unwrap().get("/samplePath").expect("/samplePath");
    let get = sample.get_get().expect("get");
    let body = get.get_request_body().expect("requestBody");
    assert!(body.get_content().unwrap().get("application/json").unwrap().get_schema().is_some());
}

#[test]
fn test_issue409() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = "openapi: 3.0.0\nservers:\n  - url: /\ninfo:\n  title: My API\n  description: It works.\n  version: 1.0.0\npaths: {}\ncomponents:\n  schemas:\n\n  # ===============================================================================\n  # Fragments\n  # ===============================================================================\n\n    ID:\n      description: An entity identifer\n      type: integer\n      format: int64\n      readOnly: true\n\n  # ===========================================================================\n  # Users\n  # ===========================================================================\n\n    User:\n      type: object\n      required:\n        - emailAddress\n      properties:\n        id:\n          $ref: \"#/components/schemas/ID\"\n        emailAddress:\n          type: string\n          format: email\n          minLength: 6\n          maxLength: 254";
    let swagger = OpenAPIV3Parser::new().read_contents(yaml, None, None).openapi.unwrap();
    assert!(swagger.get_components().unwrap().get_schemas().unwrap().get("ID").is_some());
}

#[test]
fn test_resolve_relative_paths() {
    let _g = common::setup(common::V3_MODULE);
    let _remote = mock_remote_url(&[]);
    let mut options = ParseOptions::new();
    options.resolve = true;
    let result = OpenAPIV3Parser::new().read_location("/relative-references-example/openapi.yaml", None, Some(&options));
    assert!(result.openapi.is_some());
    let validation_messages: HashSet<String> = result.messages.clone().unwrap_or_default().into_iter().collect();
    assert!(validation_messages.is_empty(), "{validation_messages:?}");
}

#[test]
fn test_resolve_relative_sibling_paths() {
    let _g = common::setup(common::V3_MODULE);
    let _remote = mock_remote_url(&[]);
    let mut options = ParseOptions::new();
    options.resolve = true;
    let result = OpenAPIV3Parser::new().read_location("/relativeParent/root/root.yaml", None, Some(&options));
    assert!(result.openapi.is_some());
    let validation_messages: HashSet<String> = result.messages.clone().unwrap_or_default().into_iter().collect();
    assert!(validation_messages.is_empty(), "{validation_messages:?}");
}
