//! Port of `io.swagger.v3.parser.util.RemoteUrlTest` (WireMock over HTTPS on a fixed port → the local
//! stub server).

mod common;

use common::wiremock::*;
use std::sync::Arc;
use swagger_parser::core::AuthorizationValue;
use swagger_parser::parser::remote_url::{clean_url, set_trust_all, url_to_string};
use swagger_parser_safe_url_resolver::{PermittedUrlsChecker, HOST_DENIED_EXCEPTION};

const WIRE_MOCK_PORT: u16 = 9999;
const EXPECTED_ACCEPTS_HEADER: &str = "application/json, application/yaml, */*";
const LOCALHOST: &str = "localhost";

/// `@BeforeMethod setUp`: trustAll, an HTTPS WireMock on 9999 (one per test; the port is freed afterwards)
fn set_up() -> WireMockServer {
    set_trust_all(true);
    WireMockServer::start(WIRE_MOCK_PORT, true)
}

fn get_url() -> String {
    format!("https://{LOCALHOST}:{WIRE_MOCK_PORT}/v2/pet/1")
}

fn setup_stub(server: &WireMockServer) -> String {
    let expected_body = "a really good body";
    server.stub_for(get(url_equal_to("/v2/pet/1")).will_return(a_response().with_body(expected_body).with_header("Content-Type", "application/json")));
    expected_body.to_string()
}

/// `PermittedUrlsCheckerAllowLocal`
fn allow_local() -> PermittedUrlsChecker {
    PermittedUrlsChecker::default().with_restricted_ip_range(Arc::new(|_| false))
}

#[test]
fn test_clean_url() {
    let _g = common::setup(common::V3_MODULE);
    assert_eq!(clean_url("http://foo/bar/com/{v2}/fun"), "http://foo/bar/com/%7Bv2%7D/fun");
    assert_eq!(
        clean_url("http://westus.dev.cognitive.microsoft.com/docs/services/563879b61984550e40cbbe8d/export?DocumentFormat=Swagger&ApiName=Face API - V1.0"),
        "http://westus.dev.cognitive.microsoft.com/docs/services/563879b61984550e40cbbe8d/export?DocumentFormat=Swagger&ApiName=Face%20API%20-%20V1.0"
    );
}

#[test]
fn test_read_a_remote_url() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up();
    let expected_body = setup_stub(&server);
    let actual_body = url_to_string(&get_url(), &[], None).unwrap();
    assert_eq!(actual_body, expected_body);
    let reqs = server.find_all(&url_equal_to("/v2/pet/1"));
    assert!(reqs.iter().any(|r| r.header("Accept") == Some(EXPECTED_ACCEPTS_HEADER)));
}

#[test]
fn test_authorization_header() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up();
    let expected_body = setup_stub(&server);
    let auth = AuthorizationValue::new("Authorization", "foobar", "header");
    let actual_body = url_to_string(&get_url(), &[auth], None).unwrap();
    assert_eq!(actual_body, expected_body);
    let reqs = server.find_all(&url_equal_to("/v2/pet/1"));
    assert!(reqs.iter().any(|r| r.header("Accept") == Some(EXPECTED_ACCEPTS_HEADER) && r.header("Authorization") == Some("foobar")));
}

#[test]
fn test_authorization_header_with_matching_url() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up();
    let expected_body = setup_stub(&server);
    let auth = AuthorizationValue::with_matcher("Authorization", "foobar", "header", Arc::new(|u| u.to_external_form().starts_with("https://localhost")));
    let actual_body = url_to_string(&get_url(), &[auth], None).unwrap();
    assert_eq!(actual_body, expected_body);
    let reqs = server.find_all(&url_equal_to("/v2/pet/1"));
    assert!(reqs.iter().any(|r| r.header("Accept") == Some(EXPECTED_ACCEPTS_HEADER) && r.header("Authorization") == Some("foobar")));
}

#[test]
fn test_authorization_header_with_non_matching_url() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up();
    let expected_body = setup_stub(&server);
    let auth = AuthorizationValue::with_matcher("Authorization", "foobar", "header", Arc::new(|_| false));
    let actual_body = url_to_string(&get_url(), &[auth], None).unwrap();
    assert_eq!(actual_body, expected_body);
    let reqs = server.find_all(&url_equal_to("/v2/pet/1"));
    assert_eq!(reqs.len(), 1);
    assert!(!reqs[0].contains_header("Authorization"));
}

/// `testConnectionTimeoutEnforced`: a connection to a non-routable port either fails fast or times out
/// within CONNECTION_TIMEOUT (+2 s); only a timeout is asserted on, as upstream.
#[test]
fn test_connection_timeout_enforced() {
    let _g = common::setup(common::V3_MODULE);
    let start = std::time::Instant::now();
    let r = url_to_string("https://httpbin.org:81/delay/60", &[], None);
    if let Err(e) = r && e.class == "java.net.SocketTimeoutException" {
        let ms = start.elapsed().as_millis() as i64;
        assert!(ms >= 30000 - 500, "Timeout was too short");
        assert!(ms <= 30000 + 2000, "Timeout was not enforced properly (took too long)");
    }
}

#[test]
fn test_too_many_redirects_throws_exception() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up();
    let mut next_path = "/redirect".to_string();
    for i in 0..6 {
        let target = format!("/redirect{}", i + 1);
        server.stub_for(get(url_equal_to(&next_path)).will_return(a_response().with_status(302).with_header("Location", &format!("{}{target}", server.base_url()))));
        next_path = target;
    }
    server.stub_for(get(url_equal_to(&next_path)).will_return(a_response().with_status(200).with_body("Final destination")));
    let start_url = format!("https://{LOCALHOST}:{WIRE_MOCK_PORT}/redirect");
    let e = url_to_string(&start_url, &[], Some(&allow_local())).unwrap_err();
    // expectedExceptions = IOException.class
    assert_eq!(e.class, "java.io.IOException");
    assert!(e.message.as_deref().unwrap_or("").contains("Too many redirects"));
}

#[test]
fn test_redirect_with_forbidden_protocol_throws_exception() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up();
    let mut next_path = "/redirect".to_string();
    for i in 0..3 {
        let target = format!("/redirect{}", i + 1);
        server.stub_for(get(url_equal_to(&next_path)).will_return(a_response().with_status(302).with_header("Location", &format!("ftp://localhost/{target}"))));
        next_path = target;
    }
    server.stub_for(get(url_equal_to(&next_path)).will_return(a_response().with_status(200).with_body("Final destination")));
    let start_url = format!("https://{LOCALHOST}:{WIRE_MOCK_PORT}/redirect");
    let e = url_to_string(&start_url, &[], Some(&allow_local())).unwrap_err();
    assert_eq!(e.class, HOST_DENIED_EXCEPTION);
}
