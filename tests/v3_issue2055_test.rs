//! Port of `io.swagger.v3.parser.test.Issue2055Test`.

mod common;

use swagger_parser::core::ParseOptions;
use swagger_parser::java::JMap;
use swagger_parser::models::*;
use swagger_parser::parser::OpenAPIV3Parser;

fn parse_at(location: &str) -> OpenAPI {
    let mut options = ParseOptions::new();
    options.resolve = true;
    let result = OpenAPIV3Parser::new().read_location(location, None, Some(&options));
    assert!(result.openapi.is_some());
    let messages = result.messages.clone().unwrap_or_default();
    assert!(messages.is_empty(), "Unexpected parser messages: {}", common::java_list(&messages));
    assert!(result.openapi.as_ref().unwrap().get_components().is_some());
    result.openapi.unwrap()
}

fn parse() -> OpenAPI {
    parse_at("src/test/resources/issue-2055/openapi.json")
}

fn component_name(r: &str) -> &str {
    &r[r.rfind('/').map(|i| i + 1).unwrap_or(0)..]
}

fn assert_local_ref_resolves<T: Clone>(r: Option<String>, component_type: &str, components: &JMap<T>) -> T {
    let r = r.expect("ref");
    assert!(r.starts_with(&format!("#/components/{component_type}/")), "Expected a local component ref: {r}");
    components.get(component_name(&r)).unwrap_or_else(|| panic!("Reference does not resolve to a component: {r}"))
}

fn contains_all(keys: &[String], expected: &[&str]) -> bool {
    expected.iter().all(|e| keys.iter().any(|k| k == e))
}

#[test]
fn external_parameters_with_same_name_keep_both_definitions() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = parse();
    let a_path = open_api.get_paths().unwrap().get("/a-path").unwrap();
    let (post, put) = (a_path.get_post().unwrap(), a_path.get_put().unwrap());

    assert_eq!(post.get_parameters().unwrap().get(0).unwrap().get_description().as_deref(), Some("There can be only 1"));
    assert_eq!(put.get_parameters().unwrap().get(0).unwrap().get_description().as_deref(), Some("There can be only 2"));

    let parameters = open_api.get_components().unwrap().get_parameters().unwrap();
    assert_eq!(parameters.len(), 10);
    assert!(contains_all(&parameters.keys(), &["limit", "limit_1"]));
    assert!(parameters.values().iter().any(|p| p.get_description().as_deref() == Some("There can be only 1")));
    assert!(parameters.values().iter().any(|p| p.get_description().as_deref() == Some("There can be only 2")));
}

const PARAMETER_LOCATIONS: &[(&str, &str, usize)] = &[("path", "pathParam", 0), ("query", "queryParam", 1), ("header", "headerParam", 2), ("cookie", "cookieParam", 3)];

#[test]
fn external_parameters_with_same_name_keep_both_definitions_for_every_location() {
    let _g = common::setup(common::V3_MODULE);
    for (location, component_base_name, parameter_index) in PARAMETER_LOCATIONS {
        let open_api = parse();
        let item = open_api.get_paths().unwrap().get("/parameter-types/{pathParam}").unwrap();
        let first = item.get_post().unwrap().get_parameters().unwrap().get(*parameter_index).unwrap();
        let second = item.get_put().unwrap().get_parameters().unwrap().get(*parameter_index).unwrap();

        assert_eq!(first.get_in().as_deref(), Some(*location));
        assert_eq!(second.get_in().as_deref(), Some(*location));
        assert_eq!(first.get_description(), Some(format!("{location} parameter from file one")));
        assert_eq!(second.get_description(), Some(format!("{location} parameter from file two")));

        let parameters = open_api.get_components().unwrap().get_parameters().unwrap();
        assert!(contains_all(&parameters.keys(), &[component_base_name, &format!("{component_base_name}_1")]), "{location}");
        assert!(parameters.values().iter().any(|p| p.get_description() == Some(format!("{location} parameter from file one"))));
        assert!(parameters.values().iter().any(|p| p.get_description() == Some(format!("{location} parameter from file two"))));
    }
}

const EXTERNAL_CONTENT_COMPONENT_TYPES: &[(&str, &str, usize)] = &[("requestBodies", "a-request", 2), ("responses", "a-response", 3)];

fn content_component_ref(operation: &Operation, component_type: &str) -> Option<String> {
    if component_type == "requestBodies" {
        return operation.get_request_body().unwrap().get_ref();
    }
    operation.get_responses().unwrap().get("200").unwrap().get_ref()
}

#[test]
fn external_content_components_with_same_name_keep_both_definitions() {
    let _g = common::setup(common::V3_MODULE);
    for (component_type, component_base_name, expected_component_count) in EXTERNAL_CONTENT_COMPONENT_TYPES {
        let open_api = parse();
        let a_path = open_api.get_paths().unwrap().get("/a-path").unwrap();
        let (post, put) = (a_path.get_post().unwrap(), a_path.get_put().unwrap());
        let post_ref = content_component_ref(&post, component_type);
        let put_ref = content_component_ref(&put, component_type);

        assert_ne!(post_ref, put_ref);

        let expected_names = [*component_base_name, &format!("{component_base_name}_1")];
        let components = open_api.get_components().unwrap();
        if *component_type == "requestBodies" {
            let m = components.get_request_bodies().unwrap();
            assert_eq!(m.len(), *expected_component_count);
            assert!(contains_all(&m.keys(), &expected_names));
            assert!(assert_local_ref_resolves(post_ref, component_type, &m).get_content().unwrap().contains_key("text/plain"));
            assert!(assert_local_ref_resolves(put_ref, component_type, &m).get_content().unwrap().contains_key("application/json"));
        } else {
            let m = components.get_responses().unwrap();
            assert_eq!(m.len(), *expected_component_count);
            assert!(contains_all(&m.keys(), &expected_names));
            assert!(assert_local_ref_resolves(post_ref, component_type, &m).get_content().unwrap().contains_key("text/plain"));
            assert!(assert_local_ref_resolves(put_ref, component_type, &m).get_content().unwrap().contains_key("application/json"));
        }
    }
}

#[test]
fn all_collision_prone_component_types_keep_distinct_definitions() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = parse_at("src/test/resources/issue-2055/all-components.yaml");
    let c = open_api.get_components().unwrap();

    let headers = c.get_headers().unwrap();
    assert!(contains_all(&headers.keys(), &["sharedHeader", "sharedHeader_1", "sharedHeader_2", "sharedHeader_3", "sharedHeader_4"]));
    assert_eq!(headers.len(), 5, "Repeated references after suffix allocation must reuse their assigned key");
    assert!(headers.values().iter().any(|h| h.get_description().as_deref() == Some("header from file one")));
    assert!(headers.values().iter().any(|h| h.get_description().as_deref() == Some("header from file two")));

    let links = c.get_links().unwrap();
    assert!(contains_all(&links.keys(), &["sharedLink", "sharedLink_1"]));
    assert_ne!(links.get("sharedLink").unwrap().get_operation_id(), links.get("sharedLink_1").unwrap().get_operation_id());

    let examples = c.get_examples().unwrap();
    assert!(contains_all(&examples.keys(), &["sharedExample", "sharedExample_1", "equalExample"]));
    assert!(!examples.contains_key("equalExample_1"));

    let callbacks = c.get_callbacks().unwrap();
    assert!(contains_all(&callbacks.keys(), &["sharedCallback", "sharedCallback_1"]));
    assert_eq!(callbacks.len(), 2, "Equivalent URI spellings must reuse the canonical callback key");
}

#[test]
fn security_schemes_retain_their_local_keys() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = parse_at("src/test/resources/issue-2055/all-components.yaml");
    let schemes = open_api.get_components().unwrap().get_security_schemes().unwrap();

    assert_eq!(schemes.len(), 2);
    assert_eq!(schemes.get("firstAuth").unwrap().get_type(), Some(SecuritySchemeType::ApiKey));
    assert_eq!(schemes.get("secondAuth").unwrap().get_type(), Some(SecuritySchemeType::Http));
}

#[test]
fn every_rewritten_local_reference_resolves_to_its_expected_component() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = parse_at("src/test/resources/issue-2055/all-components.yaml");
    let paths = open_api.get_paths().unwrap();
    let first = paths.get("/first").unwrap().get_post().unwrap();
    let second = paths.get("/second").unwrap().get_post().unwrap();
    let equivalent = paths.get("/equivalent").unwrap().get_post().unwrap();
    let c = open_api.get_components().unwrap();

    let first_request = assert_local_ref_resolves(first.get_request_body().unwrap().get_ref(), "requestBodies", &c.get_request_bodies().unwrap());
    let second_request = assert_local_ref_resolves(second.get_request_body().unwrap().get_ref(), "requestBodies", &c.get_request_bodies().unwrap());
    assert_eq!(first_request.get_description().as_deref(), Some("request from file one"));
    assert_eq!(second_request.get_description().as_deref(), Some("request from file two"));

    let first_response = assert_local_ref_resolves(first.get_responses().unwrap().get("200").unwrap().get_ref(), "responses", &c.get_responses().unwrap());
    let second_response = assert_local_ref_resolves(second.get_responses().unwrap().get("200").unwrap().get_ref(), "responses", &c.get_responses().unwrap());
    assert_eq!(first_response.get_description().as_deref(), Some("response from file one"));
    assert_eq!(second_response.get_description().as_deref(), Some("response from file two"));

    let callbacks = c.get_callbacks().unwrap();
    let first_callback = assert_local_ref_resolves(first.get_callbacks().unwrap().get("event").unwrap().get_ref(), "callbacks", &callbacks);
    let second_callback = assert_local_ref_resolves(second.get_callbacks().unwrap().get("event").unwrap().get_ref(), "callbacks", &callbacks);
    let repeated_first_callback = assert_local_ref_resolves(equivalent.get_callbacks().unwrap().get("event").unwrap().get_ref(), "callbacks", &callbacks);
    let key = "{$request.body#/callbackUrl}";
    assert!(first_callback.contains_key(key));
    assert!(second_callback.contains_key(key));
    let desc = |cb: &Callback| cb.get(key).unwrap().get_post().unwrap().get_responses().unwrap().get("204").unwrap().get_description();
    assert_eq!(desc(&first_callback).as_deref(), Some("callback from file one"));
    assert_eq!(desc(&second_callback).as_deref(), Some("callback from file two"));
    assert!(repeated_first_callback.ptr_eq(&first_callback));
}
