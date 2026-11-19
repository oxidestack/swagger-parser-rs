//! Port of `io.swagger.v3.parser.test.NetworkReferenceTest` (`@Mocked RemoteUrl`: every remote document is
//! served from the recorded expectations, never from the network).

mod common;

use common::{mock_remote_url, read_file};
use swagger_parser::core::{AuthorizationValue, ParseOptions};
use swagger_parser::models::*;
use swagger_parser::parser::OpenAPIV3Parser;

fn resolve_options() -> ParseOptions {
    let mut o = ParseOptions::new();
    o.resolve = true;
    o
}

#[test]
fn test_validate_external_refs_true_remote() {
    let _g = common::setup(common::V3_MODULE);
    let mut options = ParseOptions::new();
    options.validate_external_refs = true;
    options.resolve = true;
    let root = read_file("src/test/resources/swos-443/root.yaml");
    let reference = read_file("src/test/resources/swos-443/ref.yaml");
    let _remote = mock_remote_url(&[("http://localhost:8080/swos-443/root.yaml", &root), ("http://localhost:8080/swos-443/ref.yaml", &reference)]);

    let result = OpenAPIV3Parser::new().read_location("http://localhost:8080/swos-443/root.yaml", None, Some(&options));
    let messages = result.messages.clone().expect("messages");
    assert_eq!(messages.len(), 20, "{messages:?}");
    assert!(result.openapi.is_some());
    for m in [
        "attribute components.requestBodies.NewItem.asdasd is unexpected (./ref.yaml)",
        "attribute components.requestBodies.NewItem.descasdasdription is unexpected (./ref.yaml)",
        "attribute components.responses.GeneralError.descrsaiption is unexpected (./ref.yaml)",
        "attribute components.responses.GeneralError.asdas is unexpected (./ref.yaml)",
        "attribute components.responses.GeneralError.description is missing (./ref.yaml)",
        "attribute components.schemas.Examples.nonExpected is unexpected (./ref.yaml)",
        "attribute components.parameters.skipParam.[skip].in is not of type `[query|header|path|cookie]` (./ref.yaml)",
        "attribute components.securitySchemes.api_key.namex is unexpected (./ref.yaml)",
        "attribute components.securitySchemes.api_key.name is missing (./ref.yaml)",
        "attribute components.securitySchemes.api_key.in is not of type `cookie|header|query` (./ref.yaml)",
        "attribute components.callbacks.webhookVerificationEvent.postx is unexpected (./ref.yaml)",
        "attribute components.headers.X-Rate-Limit-Limit.descriptasdd is unexpected (./ref.yaml)",
        "attribute components.links.unsubscribe.parametersx is unexpected (./ref.yaml)",
        "attribute components.examples.response-example.summaryx is unexpected (./ref.yaml)",
        "components.examples.response-example. value and externalValue are both present (./ref.yaml)",
        "attribute components.callbacks.failed.wrongField is not of type `object` (./ref.yaml)",
        "attribute paths.~1refPet(get).responses is missing (./ref.yaml)",
        // error message in main file
        "attribute components.schemas.InvalidSchema.invalid is unexpected",
    ] {
        assert!(messages.contains(&m.to_string()), "{m}\n{messages:?}");
    }
}

#[test]
fn test_issue323() {
    let _g = common::setup(common::V3_MODULE);
    let (a, b, c, d) = (
        read_file("src/test/resources/nested-file-references/issue-323.yaml"),
        read_file("src/test/resources/nested-file-references/eventsCase9.yaml"),
        read_file("src/test/resources/nested-file-references/common/pagingWithFolderRef.yaml"),
        read_file("src/test/resources/nested-file-references/common/common2/bar.yaml"),
    );
    let _remote = mock_remote_url(&[
        ("http://localhost:8080/nested-file-references/issue-323.yaml", &a),
        ("http://localhost:8080/nested-file-references/eventsCase9.yaml", &b),
        ("http://localhost:8080/nested-file-references/common/pagingWithFolderRef.yaml", &c),
        ("http://localhost:8080/nested-file-references/common/common2/bar.yaml", &d),
    ]);

    let result = OpenAPIV3Parser::new().read_location("http://localhost:8080/nested-file-references/issue-323.yaml", Some(&[]), Some(&resolve_options()));
    let swagger = result.openapi.expect("openAPI");
    assert!(swagger.get_paths().unwrap().get("/events").is_some());
    let schemas = swagger.get_components().unwrap().get_schemas().unwrap();
    assert!(schemas.get("StatusResponse").is_some());
    assert!(schemas.get("Paging").is_some());
    assert!(schemas.get("Foobar").is_some());
}

#[test]
fn test_issue328() {
    let _g = common::setup(common::V3_MODULE);
    let (a, b, c, d) = (
        read_file("src/test/resources/nested-file-references/issue-328.yaml"),
        read_file("src/test/resources/nested-file-references/issue-328-events.yaml"),
        read_file("src/test/resources/nested-file-references/common/issue-328-paging.yaml"),
        read_file("src/test/resources/nested-file-references/common/common2/issue-328-bar.yaml"),
    );
    let _remote = mock_remote_url(&[
        ("http://localhost:8080/resources/swagger/issue-328.yaml", &a),
        ("http://localhost:8080/resources/swagger/issue-328-events.yaml", &b),
        ("http://localhost:8080/resources/swagger/common/issue-328-paging.yaml", &c),
        ("http://localhost:8080/resources/swagger/common/common2/issue-328-bar.yaml", &d),
    ]);

    let result = OpenAPIV3Parser::new().read_location("http://localhost:8080/resources/swagger/issue-328.yaml", Some(&[]), Some(&resolve_options()));
    let swagger = result.openapi.expect("openAPI");
    assert!(swagger.get_paths().unwrap().get("/events").is_some());
    let schemas = swagger.get_components().unwrap().get_schemas().unwrap();
    assert!(schemas.get("StatusResponse").is_some());
    assert!(schemas.get("Paging").is_some());
    assert!(schemas.get("Foobar").is_some());
}

#[test]
fn test_issue330() {
    let _g = common::setup(common::V3_MODULE);
    let (a, b, c, d) = (
        read_file("src/test/resources/nested-network-references/issue-330.yaml"),
        read_file("src/test/resources/nested-network-references/common/issue-330-paging.yaml"),
        read_file("src/test/resources/nested-network-references/common/issue-330-users.yaml"),
        read_file("src/test/resources/nested-network-references/common/issue-330-entities.yaml"),
    );
    let _remote = mock_remote_url(&[
        ("http://server1/resources/swagger.yaml", &a),
        ("http://server1/resources/common/paging.yaml", &b),
        ("http://server1/resources/common/users.yaml", &c),
        ("http://server2/resources/common/entities.yaml", &d),
    ]);

    let result = OpenAPIV3Parser::new().read_location("http://server1/resources/swagger.yaml", Some(&[]), Some(&resolve_options()));
    let swagger = result.openapi.expect("openAPI");
    assert!(swagger.get_paths().unwrap().get("/events").is_some());
    let schemas = swagger.get_components().unwrap().get_schemas().unwrap();
    assert!(schemas.get("Address").is_some());
    assert!(schemas.get("Paging").is_some());
    assert!(schemas.get("users").is_some());
    assert!(schemas.get("Phone").is_some());
}

#[test]
fn test_issue335() {
    let _g = common::setup(common::V3_MODULE);
    let (a, b) = (read_file("src/test/resources/nested-file-references/issue-335.json"), read_file("src/test/resources/nested-file-references/issue-335-bar.json"));
    let _remote = mock_remote_url(&[("http://server1/resources/swagger.json", &a), ("http://server1/resources/Bar.json", &b)]);

    let result = OpenAPIV3Parser::new().read_location("http://server1/resources/swagger.json", Some(&[]), Some(&resolve_options()));
    let swagger = result.openapi.expect("openAPI");
    let schemas = swagger.get_components().unwrap().get_schemas().expect("schemas");
    assert!(schemas.get("BarData").is_some());
    assert!(schemas.get("BarSettingsRequest").is_some());
}

#[test]
fn test_path_reference() {
    let _g = common::setup(common::V3_MODULE);
    let petstore = read_file("src/test/resources/petstore.yaml");
    let _remote = mock_remote_url(&[("http://petstore.swagger.io/v2/swagger.json", &petstore)]);

    let yaml = "openapi: 3.0.0\ninfo:\n  description: ''\n  version: 1.0.0\n  title: testing\npaths:\n  /foo:\n    $ref: 'http://petstore.swagger.io/v2/swagger.json#/paths/~1pet'\n  /bar:\n    $ref: 'http://petstore.swagger.io/v2/swagger.json#/paths/~1pet'\n";
    let result = OpenAPIV3Parser::new().read_contents_simple(yaml);
    assert!(result.openapi.is_some());
    assert_eq!(result.messages.as_ref().unwrap().len(), 0, "{:?}", result.messages);
    assert_eq!(result.openapi.unwrap().get_components().unwrap().get_schemas().unwrap().len(), 3);
}

#[test]
fn test_issue411() {
    let _g = common::setup(common::V3_MODULE);
    let auths = vec![AuthorizationValue::new("Authorization", "OMG_SO_SEKR3T", "header")];
    let (a, b) = (read_file("src/test/resources/nested-network-references/issue-411-server.yaml"), read_file("src/test/resources/nested-network-references/issue-411-remote2.yaml"));
    let remote = mock_remote_url(&[("http://remote1/resources/swagger.yaml", &a), ("http://remote2/resources/foo", &b)]);

    let mut options = ParseOptions::new();
    options.resolve = true;
    options.resolve_responses = true;
    let result = OpenAPIV3Parser::new().read_location("http://remote1/resources/swagger.yaml", Some(&auths), Some(&options));

    // the expectations are recorded with the auths list: both calls receive it
    for (_, a) in remote.calls() {
        assert_eq!(a, vec![(Some("Authorization".to_string()), Some("OMG_SO_SEKR3T".to_string()), Some("header".to_string()))]);
    }
    let swagger = result.openapi.unwrap();
    let health = swagger.get_paths().unwrap().get("/health").expect("/health");
    assert_eq!(health.get_get().unwrap().get_parameters().unwrap().len(), 0);
    let response_ref = health.get_get().unwrap().get_responses().unwrap().get("200").unwrap().get_content().unwrap().get("*/*").unwrap().get_schema().unwrap();
    assert!(response_ref.get_ref().is_some());
    assert_eq!(response_ref.get_ref().as_deref(), Some("#/components/schemas/Success"));
    assert!(swagger.get_components().unwrap().get_schemas().unwrap().get("Success").is_some());

    let stuff = swagger.get_paths().unwrap().get("/stuff").unwrap().get_get().unwrap();
    let param = stuff.get_parameters().unwrap().get(0).unwrap();
    assert_eq!(param.get_in().as_deref(), Some("query"));
    assert_eq!(param.get_name().as_deref(), Some("skip"));

    let response = stuff.get_responses().unwrap().get("200").expect("200");
    // instanceof StringSchema
    assert_eq!(response.get_content().unwrap().get("*/*").unwrap().get_schema().unwrap().kind(), SchemaKind::String);

    let error = stuff.get_responses().unwrap().get("400").expect("400");
    let error_prop = error.get_content().unwrap().get("*/*").unwrap().get_schema().expect("schema");
    assert!(error_prop.get_ref().is_some());
    assert_eq!(error_prop.get_ref().as_deref(), Some("#/components/schemas/Error"));
    assert!(swagger.get_components().unwrap().get_schemas().unwrap().get("Error").is_some());
}

#[test]
fn test_issue742() {
    let _g = common::setup(common::V3_MODULE);
    let json = read_file("src/test/resources/issue-742.json");
    let _remote = mock_remote_url(&[("http://www.example.io/one/two/swagger.json", &json)]);

    let result = OpenAPIV3Parser::new().read_location("http://www.example.io/one/two/swagger.json", Some(&[]), Some(&resolve_options()));
    let open_api = result.openapi.expect("openAPI");
    let servers = open_api.get_servers().expect("servers");
    assert_eq!(4, servers.len());
    let s = |i: usize| servers.get(i).unwrap();
    assert_eq!(Some("An absolute path".to_string()), s(0).get_description());
    assert_eq!(Some("https://api.absolute.org/v2".to_string()), s(0).get_url());
    assert_eq!(Some("Server relative to root path".to_string()), s(1).get_description());
    assert_eq!(Some("http://www.example.io/api/v2".to_string()), s(1).get_url());
    assert_eq!(Some("Server relative path 1".to_string()), s(2).get_description());
    assert_eq!(Some("http://www.example.io/one/two/path/v2".to_string()), s(2).get_url());
    assert_eq!(Some("Server relative path 2".to_string()), s(3).get_description());
    assert_eq!(Some("http://www.example.io/one/v2".to_string()), s(3).get_url());
}
