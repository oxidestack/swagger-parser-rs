//! Port of `io.swagger.v3.parser.processors.ExternalRefProcessorTest` (`@Injectable ResolverCache`, `OpenAPI`
//! and `Schema mockedModel`; `@Mocked RemoteUrl`).

mod common;

use common::mock_cache::{dyn_cache, CacheCall, MockCache};
use std::rc::Rc;
use swagger_parser::core::ParseOptions;
use swagger_parser::java::{JList, JMap};
use swagger_parser::models::*;
use swagger_parser::parser::processors::ExternalRefProcessor;
use swagger_parser::parser::ref_utils::RefFormat;
use swagger_parser::parser::resolver_cache::{ComponentKind, ResolverCache};
use swagger_parser::parser::OpenAPIV3Parser;

#[test]
fn test_process_ref_to_external_definition_no_name_conflict() {
    let _g = common::setup(common::V3_MODULE);
    let (cache, open_api) = (MockCache::new(), OpenAPI::new());
    let mocked_model = Schema::new();
    let r = "http://my.company.com/path/to/file.json#/foo/bar";
    let ref_format = RefFormat::Url;
    cache.expect_load_ref(r, ref_format, Some(mocked_model.clone()));

    let new_ref = ExternalRefProcessor::new(dyn_cache(&cache), &open_api).process_ref_to_external_schema(r, ref_format).unwrap();
    assert_eq!(new_ref.as_deref(), Some("bar"));

    // the recorded expectations (each expected at least once)
    assert!(cache.count(&CacheCall::GetRenamedRef(Some(r.into()))) >= 1);
    assert!(cache.count(&CacheCall::LoadRef(r.into(), ref_format, ComponentKind::Schema)) >= 1);
    assert!(cache.count(&CacheCall::PutRenamedRef(r.into(), "bar".into())) >= 1);
    assert!(open_api.get_components().unwrap().get_schemas().unwrap().get("bar").unwrap().ptr_eq(&mocked_model));
    assert!(cache.count(&CacheCall::AddReferencedKey("bar".into())) >= 1);
}

#[test]
fn test_security_schemes_with_same_external_name_receive_distinct_keys() {
    let _g = common::setup(common::V3_MODULE);
    let cache = MockCache::new();
    let first_ref = "https://example.test/first.yaml#/sharedAuth";
    let second_ref = "https://example.test/second.yaml#/sharedAuth";
    let first = SecurityScheme::new().type_(SecuritySchemeType::ApiKey).name("X-API-Key").in_(SecuritySchemeIn::Header);
    let second = SecurityScheme::new().type_(SecuritySchemeType::Http).scheme("bearer");
    let tested = OpenAPI::new().components(Components::new());
    cache.expect_load_ref(first_ref, RefFormat::Url, Some(first.clone()));
    cache.expect_load_ref(second_ref, RefFormat::Url, Some(second.clone()));

    let processor = ExternalRefProcessor::new(dyn_cache(&cache), &tested);
    assert_eq!(processor.process_ref_to_external_security_scheme(first_ref, RefFormat::Url).unwrap().as_deref(), Some("sharedAuth"));
    assert_eq!(processor.process_ref_to_external_security_scheme(second_ref, RefFormat::Url).unwrap().as_deref(), Some("sharedAuth_1"));
    let schemes = tested.get_components().unwrap().get_security_schemes().unwrap();
    assert_eq!(schemes.get("sharedAuth"), Some(first));
    assert_eq!(schemes.get("sharedAuth_1"), Some(second));
}

#[test]
fn test_response_allocator_examines_occupied_suffixes_without_overwriting_placeholder() {
    let _g = common::setup(common::V3_MODULE);
    let cache = MockCache::new();
    let r = "https://example.test/incoming.yaml#/sharedResponse";
    let placeholder = ApiResponse::new().ref_("https://example.test/other.yaml#/sharedResponse");
    let first_suffix = ApiResponse::new().description("first suffix");
    let second_suffix = ApiResponse::new().description("second suffix");
    let incoming = ApiResponse::new().description("incoming");
    let components = Components::new()
        .add_responses("sharedResponse", placeholder.clone())
        .add_responses("sharedResponse_1", first_suffix)
        .add_responses("sharedResponse_2", second_suffix);
    let tested = OpenAPI::new().components(components);
    cache.expect_load_ref(r, RefFormat::Url, Some(incoming.clone()));

    let assigned = ExternalRefProcessor::new(dyn_cache(&cache), &tested).process_ref_to_external_response(r, RefFormat::Url).unwrap();

    assert_eq!(assigned.as_deref(), Some("sharedResponse_3"));
    let responses = tested.get_components().unwrap().get_responses().unwrap();
    assert!(responses.get("sharedResponse").unwrap().ptr_eq(&placeholder));
    assert_eq!(placeholder.get_ref().as_deref(), Some("https://example.test/other.yaml#/sharedResponse"));
    assert!(responses.get("sharedResponse_3").unwrap().ptr_eq(&incoming));
}

#[test]
fn test_equal_resolved_responses_reuse_one_key() {
    let _g = common::setup(common::V3_MODULE);
    let cache = MockCache::new();
    let r = "https://example.test/incoming.yaml#/sharedResponse";
    let existing = ApiResponse::new().description("same response");
    let incoming = ApiResponse::new().description("same response");
    let tested = OpenAPI::new().components(Components::new().add_responses("sharedResponse", existing.clone()));
    cache.expect_load_ref(r, RefFormat::Url, Some(incoming));

    let assigned = ExternalRefProcessor::new(dyn_cache(&cache), &tested).process_ref_to_external_response(r, RefFormat::Url).unwrap();

    assert_eq!(assigned.as_deref(), Some("sharedResponse"));
    let responses = tested.get_components().unwrap().get_responses().unwrap();
    assert_eq!(responses.len(), 1);
    assert!(responses.get("sharedResponse").unwrap().ptr_eq(&existing));
}

#[test]
fn test_equivalent_response_placeholder_uses_canonical_reference_identity() {
    let _g = common::setup(common::V3_MODULE);
    let cache = MockCache::new();
    let r = "https://example.test/responses/common.yaml#/sharedResponse";
    let equivalent_ref = "https://example.test/responses/../responses/common.yaml#/sharedResponse";
    let placeholder = ApiResponse::new().ref_(equivalent_ref);
    let incoming = ApiResponse::new().description("resolved response");
    let tested = OpenAPI::new().components(Components::new().add_responses("sharedResponse", placeholder));
    cache.expect_load_ref(r, RefFormat::Url, Some(incoming.clone()));
    cache.expect_refs_are_equivalent(equivalent_ref, r, true);

    let assigned = ExternalRefProcessor::new(dyn_cache(&cache), &tested).process_ref_to_external_response(r, RefFormat::Url).unwrap();

    assert_eq!(assigned.as_deref(), Some("sharedResponse"));
    let responses = tested.get_components().unwrap().get_responses().unwrap();
    assert_eq!(responses.len(), 1);
    assert!(responses.get("sharedResponse").unwrap().ptr_eq(&incoming));
}

#[test]
fn test_non_schema_component_names_remain_case_sensitive() {
    let _g = common::setup(common::V3_MODULE);
    let cache = MockCache::new();
    let r = "https://example.test/responses.yaml#/SharedResponse";
    let lower_case = ApiResponse::new().description("lower case key");
    let incoming = ApiResponse::new().description("upper case key");
    let tested = OpenAPI::new().components(Components::new().add_responses("sharedResponse", lower_case.clone()));
    cache.expect_load_ref(r, RefFormat::Url, Some(incoming.clone()));

    let assigned = ExternalRefProcessor::new(dyn_cache(&cache), &tested).process_ref_to_external_response(r, RefFormat::Url).unwrap();

    assert_eq!(assigned.as_deref(), Some("SharedResponse"));
    let responses = tested.get_components().unwrap().get_responses().unwrap();
    assert_eq!(responses.len(), 2);
    assert!(responses.get("sharedResponse").unwrap().ptr_eq(&lower_case));
    assert!(responses.get("SharedResponse").unwrap().ptr_eq(&incoming));
}

#[test]
fn test_failed_response_load_returns_original_ref() {
    let _g = common::setup(common::V3_MODULE);
    let cache = MockCache::new();
    let r = "https://example.test/missing.yaml#/MissingResponse";
    let tested = OpenAPI::new();
    cache.expect_load_ref::<ApiResponse>(r, RefFormat::Url, None);

    assert_eq!(ExternalRefProcessor::new(dyn_cache(&cache), &tested).process_ref_to_external_response(r, RefFormat::Url).unwrap().as_deref(), Some(r));
}

#[test]
fn test_recursive_external_schema_reuses_rename_cache_entry() {
    let _g = common::setup(common::V3_MODULE);
    let cache = MockCache::new();
    let r = "schemas.yaml#/Recursive";
    let recursive = Schema::new().ref_(r);
    let tested = OpenAPI::new().components(Components::new());
    // cache.getRenamedRef(ref); returns(null, "Recursive")
    cache.expect_renamed_ref(r, &[None, Some("Recursive")]);
    cache.expect_load_ref(r, RefFormat::Relative, Some(recursive.clone()));

    let assigned = ExternalRefProcessor::new(dyn_cache(&cache), &tested).process_ref_to_external_schema(r, RefFormat::Relative).unwrap();

    assert_eq!(assigned.as_deref(), Some("Recursive"));
    let schemas = tested.get_components().unwrap().get_schemas().unwrap();
    assert_eq!(schemas.len(), 1);
    assert!(schemas.get("Recursive").unwrap().ptr_eq(&recursive));
    assert_eq!(recursive.get_ref().as_deref(), Some("#/components/schemas/Recursive"));
}

#[test]
fn test_schema_lookup_prefers_exact_key_before_case_insensitive_fallback() {
    let _g = common::setup(common::V3_MODULE);
    let cache = MockCache::new();
    let r = "https://example.test/schemas.yaml#/Pet";
    let lower_case_schema = Schema::new().add_properties("lower", Schema::of(SchemaKind::String));
    let exact_schema = Schema::new().add_properties("exact", Schema::of(SchemaKind::String));
    let incoming = Schema::new().add_properties("exact", Schema::of(SchemaKind::String));
    let tested = OpenAPI::new().components(Components::new().add_schemas("pet", lower_case_schema.clone()).add_schemas("Pet", exact_schema.clone()));
    cache.expect_load_ref(r, RefFormat::Url, Some(incoming));

    let assigned = ExternalRefProcessor::new(dyn_cache(&cache), &tested).process_ref_to_external_schema(r, RefFormat::Url).unwrap();

    assert_eq!(assigned.as_deref(), Some("Pet"));
    let schemas = tested.get_components().unwrap().get_schemas().unwrap();
    assert_eq!(schemas.len(), 2);
    assert!(schemas.get("Pet").unwrap().ptr_eq(&exact_schema));
    assert!(schemas.get("pet").unwrap().ptr_eq(&lower_case_schema));
}

#[test]
fn test_nested_external_refs() {
    let _g = common::setup(common::V3_MODULE);
    let cache = MockCache::new();
    let ref_format = RefFormat::Url;
    let tested = OpenAPI::new();

    let customer_url = "http://my.company.com/path/to/customer.json#/definitions/Customer";
    let customer_model = Schema::new();
    let cust_props: JMap<Schema> = JMap::new_hash();
    let address = Schema::new();
    let address_url = "http://my.company.com/path/to/address.json#/definitions/Address";
    address.set_ref(address_url);
    cust_props.put("Address", address);

    let loyalty_url = "#/definitions/LoyaltyScheme";
    let loyalty_prop = Schema::new();
    loyalty_prop.set_ref(loyalty_url);
    loyalty_prop.set_name("LoyaltyCardNumber");
    loyalty_prop.set_required(JList::from_vec(vec!["LoyaltyCardNumber".to_string()]));
    cust_props.put("Loyalty", loyalty_prop);
    customer_model.set_properties(cust_props);

    let address_model = Schema::new();
    let address_props: JMap<Schema> = JMap::new_hash();
    let contact = Schema::new();
    let contact_url = "http://my.company.com/path/to/Contact.json#/definitions/Contact";
    contact.set_ref(contact_url);
    address_props.put("Contact", contact);
    address_model.set_properties(address_props);

    let contact_model = Schema::new();
    let contact_prop = Schema::of(SchemaKind::String);
    contact_prop.set_name("PhoneNumber");
    contact_prop.set_required(JList::from_vec(vec!["PhoneNumber".to_string()]));
    let contact_props: JMap<Schema> = JMap::new_hash();
    contact_props.put("PhoneNumber", contact_prop);
    contact_model.set_properties(contact_props);

    cache.expect_load_ref(customer_url, ref_format, Some(customer_model));
    cache.expect_load_ref(address_url, ref_format, Some(address_model));
    cache.expect_load_ref(contact_url, ref_format, Some(contact_model));

    ExternalRefProcessor::new(dyn_cache(&cache), &tested).process_ref_to_external_schema(customer_url, ref_format).unwrap();

    // cache.loadRef(customerURL, refFormat, Schema.class); times = 1 (the others at least once)
    assert_eq!(cache.count(&CacheCall::LoadRef(customer_url.into(), ref_format, ComponentKind::Schema)), 1);
    assert!(cache.count(&CacheCall::LoadRef(address_url.into(), ref_format, ComponentKind::Schema)) >= 1);
    assert!(cache.count(&CacheCall::LoadRef(contact_url.into(), ref_format, ComponentKind::Schema)) >= 1);
    let schemas = tested.get_components().unwrap().get_schemas().unwrap();
    assert!(schemas.get("Customer").is_some());
    assert!(schemas.get("Contact").is_some());
    assert!(schemas.get("Address").is_some());
}

#[test]
fn test_relative_ref_including_url_ref() {
    let _g = common::setup(common::V3_MODULE);
    let ref_format = RefFormat::Relative;
    let url = "https://my.example.remote.url.com/globals.yaml";
    let expected_result = "components:\n  schemas:\n    link-object:\n      type: object\n      additionalProperties:\n        \"$ref\": \"#/components/schemas/rel-data\"\n    rel-data:\n      type: object\n      required:\n      - href\n      properties:\n        href:\n          type: string\n        note:\n          type: string\n    result:\n      type: object\n      properties:\n        name:\n          type: string\n        _links:\n          \"$ref\": \"#/components/schemas/link-object\"\n";
    let remote = common::mock_remote_url(&[(url, expected_result)]);

    let mocked_open_api = OpenAPI::new();
    mocked_open_api.set_components(Components::new());
    mocked_open_api.get_components().unwrap().set_schemas(JMap::new_hash());
    let mocked_resolver_cache = Rc::new(ResolverCache::new(Some(&mocked_open_api), None, None).unwrap());

    let processor = ExternalRefProcessor::new(mocked_resolver_cache, &mocked_open_api);
    processor.process_ref_to_external_schema("./relative-with-url/relative-with-url.yaml#/relative-with-url", ref_format).unwrap();

    // RemoteUrl.urlToString(url, null, any); times = 1
    assert_eq!(remote.count(url), 1);
    assert_eq!(remote.calls().len(), 1);
    let schemas = mocked_open_api.get_components().unwrap().get_schemas().unwrap();
    let rel = schemas.get("relative-with-url").unwrap();
    assert_eq!(rel.get_properties().unwrap().get("Foo").unwrap().get_ref().as_deref(), Some("https://my.example.remote.url.com/globals.yaml#/components/schemas/link-object"));
    assert!(schemas.contains_key("link-object"));
    assert!(schemas.contains_key("rel-data"));
    assert_eq!(rel.get_properties().unwrap().get("Bar").unwrap().get_ref().as_deref(), Some("./relative-with-url/relative-with-local.yaml#/relative-same-file"));
}

fn resolve(location: &str) -> OpenAPI {
    let mut options = ParseOptions::new();
    options.resolve = true;
    OpenAPIV3Parser::new().read_location(location, None, Some(&options)).openapi.unwrap()
}

#[test]
fn test_handle_composed_schemas_in_array_items() {
    let _g = common::setup(common::V3_MODULE);
    let _remote = common::mock_remote_url(&[]);
    let components = resolve("issue-2071/openapi.yaml").get_components().unwrap().get_schemas().unwrap();
    assert_eq!(components.len(), 5);
    for k in ["Response", "ProductRow", "ProductRowType", "OrderRow", "OrderRowType"] {
        assert!(components.contains_key(k), "{k}");
    }
}

#[test]
fn test_handle_inline_refs_in_composed_schemas() {
    let _g = common::setup(common::V3_MODULE);
    let _remote = common::mock_remote_url(&[]);
    let components = resolve("issue-2104/openapi.yaml").get_components().unwrap().get_schemas().unwrap();
    assert_eq!(components.len(), 4);
    for k in ["ResponseAllOf", "ResponseOneOf", "ResponseAnyOf", "Product"] {
        assert!(components.contains_key(k), "{k}");
    }
    let product = |list: Option<JList<Schema>>| list.unwrap().get(1).unwrap().get_properties().unwrap().get("product").unwrap();
    assert_eq!(product(components.get("ResponseAllOf").unwrap().get_all_of()).get_ref().as_deref(), Some("#/components/schemas/Product"));
    assert_eq!(product(components.get("ResponseOneOf").unwrap().get_one_of()).get_ref().as_deref(), Some("#/components/schemas/Product"));
    assert_eq!(product(components.get("ResponseAnyOf").unwrap().get_any_of()).get_ref().as_deref(), Some("#/components/schemas/Product"));
}

#[test]
fn test_additional_properties_reference_resolution() {
    let _g = common::setup(common::V3_MODULE);
    let _remote = common::mock_remote_url(&[]);
    let open_api = resolve("src/test/resources/issue-2218/main.yaml");
    let schemas = open_api.get_components().unwrap().get_schemas();
    assert!(schemas.is_some(), "Schemas should not be null");
    let schemas = schemas.unwrap();
    assert!(schemas.contains_key("FlagsOfFlags"), "FlagsOfFlags schema should be resolved and available in components");
    assert!(schemas.contains_key("Flags"), "Flags schema should be resolved and available in components");
    assert!(schemas.contains_key("Flag"), "Flag schema should be resolved and available in components");
}
