//! Port of `io.swagger.v3.parser.test.OpenAPIV3RefTest` (a JUnit 4 class: `@Before parseOASSpec` runs before
//! every test).

mod common;

use std::collections::HashSet;
use swagger_parser::core::ParseOptions;
use swagger_parser::models::OpenAPI;
use swagger_parser::parser::OpenAPIV3Parser;

/// `Components.COMPONENTS_SCHEMAS_REF`
const COMPONENTS_SCHEMAS_REF: &str = "#/components/schemas/";

/// `@Before parseOASSpec`
fn parse_oas_spec() -> Option<OpenAPI> {
    let options = ParseOptions { resolve: true, ..Default::default() };
    OpenAPIV3Parser::new().read_with(Some("oas3-refs-test/openapi.json"), None, Some(&options))
}

#[test]
fn test_ref_containing_dot() {
    let _g = common::setup(common::V3_MODULE);
    let _oas = parse_oas_spec();
    let options = ParseOptions { resolve: true, ..Default::default() };
    let result = OpenAPIV3Parser::new().read_location("resolve-dot-containing-ref/standaloneSpec.yaml", None, Some(&options));
    let open_api = result.openapi.clone().unwrap();
    assert!(result.messages.as_ref().unwrap().is_empty(), "No error messages should be present");
    let params = open_api.get_paths().unwrap().get("/endpoint").unwrap().get_get().unwrap().get_parameters().unwrap();
    assert_eq!(params.get(0).unwrap().get_name().as_deref(), Some("FbtPrincipalIdentity"));
}

#[test]
fn test_external_ref_containing_dot() {
    let _g = common::setup(common::V3_MODULE);
    let _oas = parse_oas_spec();
    let options = ParseOptions { resolve: true, ..Default::default() };
    let result = OpenAPIV3Parser::new().read_location("resolve-dot-containing-ref/externalRefSpec.yaml", None, Some(&options));
    let open_api = result.openapi.clone().unwrap();
    assert!(result.messages.as_ref().unwrap().is_empty(), "No error messages should be present");
    let params = open_api.get_paths().unwrap().get("/endpoint").unwrap().get_get().unwrap().get_parameters().unwrap();
    assert_eq!(params.get(0).unwrap().get_name().as_deref(), Some("FbtPrincipalIdentity"));
}

#[test]
fn test_parameter_example_ref_processed() {
    let _g = common::setup(common::V3_MODULE);
    let oas = parse_oas_spec().unwrap();
    let param_name = "correlation_id";
    let param_examples = oas.get_components().unwrap().get_parameters().unwrap().get(param_name).unwrap().get_examples().unwrap();
    assert_eq!(param_examples.len(), 1, "Parameter has an example");
    assert!(
        param_examples.values().iter().all(|e| e.get_ref().unwrap().eq_ignore_ascii_case(&format!("#/components/examples/{param_name}"))),
        "Examples are referenced"
    );
    assert_eq!(
        oas.get_components().unwrap().get_examples().unwrap().get(param_name).unwrap().get_value(),
        Some(jackson::JavaObject::string("7758b780aaaca")),
        "Examples are processed"
    );
}

#[test]
fn test_discriminator_mapping_refs_updated() {
    let _g = common::setup(common::V3_MODULE);
    let oas = parse_oas_spec().unwrap();
    let req_schema = oas
        .get_paths()
        .unwrap()
        .values()
        .first()
        .and_then(|path| path.get_post().unwrap().get_request_body().unwrap().get_content().unwrap().get("application/json"))
        .and_then(|m| m.get_schema())
        .expect("Path not processed!");
    let discriminator = req_schema.get_discriminator().unwrap().get_mapping().unwrap().values();
    let all_ofs: HashSet<String> = req_schema.get_any_of().unwrap().to_vec().iter().map(|s| s.get_ref().unwrap()).collect();
    assert!(all_ofs.iter().all(|s| s.contains(COMPONENTS_SCHEMAS_REF)), "Schema mappings are processed");
    let all_ofs_names: HashSet<String> = all_ofs.iter().map(|of| of.split("/schemas/").nth(1).unwrap().to_string()).collect();
    let discriminator_names: HashSet<String> = discriminator.iter().map(|of| of.split("/schemas/").nth(1).unwrap().replace(".json", "")).collect();
    assert!(discriminator_names.is_subset(&all_ofs_names), "Discriminator mappings are updated");
}

#[test]
fn test_callback_ref() {
    let _g = common::setup(common::V3_MODULE);
    let oas = parse_oas_spec().unwrap();
    let callbacks = oas.get_components().unwrap().get_callbacks().unwrap();
    assert_eq!(callbacks.len(), 1, "Callbacks processed");
    let cb_operation = callbacks.get("vaccination_complete").unwrap().get("{$request.body#/callback_url}/pets/{$request.path.id}/vaccinations").unwrap().get_put();
    let cb_operation = cb_operation.expect("cbOperation");
    assert_eq!(
        cb_operation.get_request_body().unwrap().get_content().unwrap().get("application/json").unwrap().get_schema().unwrap().get_ref(),
        Some(format!("{COMPONENTS_SCHEMAS_REF}vaccination_record")),
        "Callback Request body processed"
    );
    assert_eq!(
        cb_operation.get_responses().unwrap().get("400").unwrap().get_content().unwrap().get("application/json").unwrap().get_schema().unwrap().get_ref(),
        Some(format!("{COMPONENTS_SCHEMAS_REF}error")),
        "Callback responses Processed"
    );
}

#[test]
fn test_composed_array_items_ref() {
    let _g = common::setup(common::V3_MODULE);
    let oas = parse_oas_spec().unwrap();
    let schemas = oas.get_components().unwrap().get_schemas().unwrap();
    let adoption_request = schemas.get("adoption_request").unwrap();
    assert_eq!(adoption_request.get_title().as_deref(), Some("Adoption Request"), "Processed ref Schemas");
    assert_eq!(
        adoption_request.get_properties().unwrap().get("adopter_alias").unwrap().get_ref(),
        Some(format!("{COMPONENTS_SCHEMAS_REF}alias_array")),
        "Processed ref added to Schemas"
    );
    let adopter_alias = schemas.get("alias_array").unwrap();
    assert_eq!(adopter_alias.get_title().as_deref(), Some("AdopterAlias"), "Processed Schemas");
    assert_eq!(adopter_alias.get_items().unwrap().get_all_of().unwrap().len(), 2, "Processed schemas items");
}
