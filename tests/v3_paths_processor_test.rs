//! Port of `io.swagger.v3.parser.processors.PathsProcessorTest`. `new PathsProcessor(null, openAPI)`: the
//! cache is never used by these methods, so an empty mock cache stands in for `null`.

mod common;

use common::mock_cache::{dyn_cache, MockCache};
use swagger_parser::core::{ParseOptions, SwaggerParseResult};
use swagger_parser::models::*;
use swagger_parser::parser::processors::external_ref::join;
use swagger_parser::parser::processors::PathsProcessor;
use swagger_parser::parser::OpenAPIV3Parser;

fn processor() -> PathsProcessor {
    PathsProcessor::new(dyn_cache(&MockCache::new()), &OpenAPI::new())
}

const REBASED_REFERENCES: &[(&str, &str, &str)] = &[
    ("./sub-dir/params.json", "./sub-dir2/p.json", "./sub-dir/sub-dir2/p.json"),
    ("./sub-dir/params.json", "sub-dir2/p.json", "./sub-dir/sub-dir2/p.json"),
    ("./sub-dir/params.json", "../parameters/p.json", "./parameters/p.json"),
    ("./sub-dir/params.json", "p.json#/components/parameters/Foo", "./sub-dir/p.json#/components/parameters/Foo"),
    ("paths/users.yaml", "../parameters/page.yaml", "parameters/page.yaml"),
    ("product/product-api.yaml", "product-components.yaml#/components/parameters/id", "product/product-components.yaml#/components/parameters/id"),
    ("./sub-dir/params.json", "https://example.com/p.json", "https://example.com/p.json"),
    ("./sub-dir/params.json", "http://example.com/p.json#/Foo", "http://example.com/p.json#/Foo"),
    ("./sub-dir/params.json", "file:/tmp/p.json#/Foo", "file:/tmp/p.json#/Foo"),
];

const URI_ILLEGAL_REFERENCES: &[&str] = &["\\", "%", "[", "]", "{", "}", "^", "`", "|", "<", ">", "\"", " "];

const ABSOLUTE_REFERENCES_WITH_URI_ILLEGAL_CHARACTERS: &[&str] =
    &["/shared params/p.yaml", "C:\\shared params\\p.yaml", "\\\\server\\shared params\\p.yaml", "https://example .com/p.yaml"];

const BARE_RELATIVE_REFS_REBASED_BY_COMPUTE_REF: &[(&str, &str, &str)] = &[
    ("schemas/product.yaml", "api/path-item.yaml", "api/schemas/product.yaml"),
    ("examples/product.yaml", "api/path-item.yaml", "api/examples/product.yaml"),
    ("product-components.yaml", "product/product-api.yaml", "product/product-components.yaml"),
];

#[test]
fn test_compute_ref_rebases_against_containing_document() {
    let _g = common::setup(common::V3_MODULE);
    for (base, r, expected) in REBASED_REFERENCES {
        assert_eq!(processor().compute_ref(r, base), *expected, "{base} {r}");
    }
}

#[test]
fn test_compute_ref_rebases_invalid_child_reference() {
    let _g = common::setup(common::V3_MODULE);
    assert_eq!(processor().compute_ref("invalid ref.yaml", "./sub-dir/params.json"), "./sub-dir/invalid ref.yaml");
}

#[test]
fn test_compute_ref_rebases_uri_illegal_characters() {
    let _g = common::setup(common::V3_MODULE);
    for character in URI_ILLEGAL_REFERENCES {
        let r = format!("../shared{character}params/p.yaml#/components/parameters/Foo");
        assert_eq!(processor().compute_ref(&r, "./sub-dir/params.json"), format!("./shared{character}params/p.yaml#/components/parameters/Foo"), "{character}");
    }
}

#[test]
fn test_compute_ref_falls_back_when_containing_document_is_not_a_valid_uri() {
    let _g = common::setup(common::V3_MODULE);
    assert_eq!(processor().compute_ref("../parameters/p.yaml", "./path items/users.yaml"), "./parameters/p.yaml");
}

#[test]
fn test_compute_ref_falls_back_when_containing_document_has_no_directory() {
    let _g = common::setup(common::V3_MODULE);
    assert_eq!(processor().compute_ref("parameters.yaml", "path item.yaml"), "parameters.yaml");
}

#[test]
fn test_compute_ref_preserves_absolute_references_with_uri_illegal_characters() {
    let _g = common::setup(common::V3_MODULE);
    for r in ABSOLUTE_REFERENCES_WITH_URI_ILLEGAL_CHARACTERS {
        assert_eq!(processor().compute_ref(r, "./sub-dir/params.json"), *r);
    }
}

#[test]
fn test_external_ref_join_falls_back_for_uri_illegal_child_reference() {
    let _g = common::setup(common::V3_MODULE);
    assert_eq!(
        join(Some("nested/api.yaml#/paths/~1products"), Some("../shared schemas/product.yaml#/components/schemas/Product")).as_deref(),
        Some("shared schemas/product.yaml#/components/schemas/Product")
    );
}

#[test]
fn test_external_ref_join_preserves_query_only_and_fragment_only_references() {
    let _g = common::setup(common::V3_MODULE);
    assert_eq!(join(Some("path items/api.yaml?version=1#/paths/~1products"), Some("?version=2")).as_deref(), Some("path items/api.yaml?version=2"));
    assert_eq!(
        join(Some("path items/api.yaml?version=1#/paths/~1products"), Some("#/components/schemas/Product")).as_deref(),
        Some("path items/api.yaml?version=1#/components/schemas/Product")
    );
}

#[test]
fn test_external_ref_join_preserves_null_compatibility() {
    let _g = common::setup(common::V3_MODULE);
    assert_eq!(join(Some("source.yaml"), None).as_deref(), Some("source.yaml"));
    assert_eq!(join(None, Some("child.yaml")), None);
}

#[test]
fn test_fragment_only_parameter_ref_uses_external_path_item_document() {
    let _g = common::setup(common::V3_MODULE);
    let parameter = Parameter::new().ref_("#/components/parameters/Foo");
    processor().update_refs_parameter(&parameter, "product/product-api.yaml");
    assert_eq!(parameter.get_ref().as_deref(), Some("product/product-api.yaml#/components/parameters/Foo"));
}

#[test]
fn test_internal_schema_ref_keeps_root_document_compatibility() {
    let _g = common::setup(common::V3_MODULE);
    let schema = Schema::new().ref_("#/components/schemas/Foo");
    processor().update_refs_schema(&schema, "product/product-api.yaml");
    assert_eq!(schema.get_ref().as_deref(), Some("#/components/schemas/Foo"));
}

fn parse(location: &str) -> SwaggerParseResult {
    let mut options = ParseOptions::new();
    options.resolve = true;
    OpenAPIV3Parser::new().read_location(location, None, Some(&options))
}

fn parse_fully(location: &str) -> SwaggerParseResult {
    let mut options = ParseOptions::new();
    options.resolve = true;
    options.resolve_fully = true;
    OpenAPIV3Parser::new().read_location(location, None, Some(&options))
}

fn parameter(result: &SwaggerParseResult, path: &str) -> Parameter {
    result.openapi.as_ref().unwrap().get_paths().unwrap().get(path).unwrap().get_get().unwrap().get_parameters().unwrap().get(0).unwrap()
}

fn assert_parameter(parameter: Option<Parameter>, name: &str, in_: &str, schema_type: &str) {
    let parameter = parameter.expect("parameter");
    assert_eq!(parameter.get_name().as_deref(), Some(name));
    assert_eq!(parameter.get_in().as_deref(), Some(in_));
    let schema = parameter.get_schema().expect("schema");
    assert_eq!(schema.get_type().as_deref(), Some(schema_type));
}

fn assert_no_relative_ref_load_failure(result: &SwaggerParseResult) {
    assert!(result.openapi.is_some());
    for message in result.messages.iter().flatten() {
        assert!(!message.contains("Unable to load RELATIVE ref"), "{message}");
    }
}

fn response_schema(result: &SwaggerParseResult, path: &str) -> Option<Schema> {
    result.openapi.as_ref().unwrap().get_paths().unwrap().get(path).unwrap().get_get().unwrap().get_responses().unwrap().get("200").unwrap().get_content().unwrap().get("application/json").unwrap().get_schema()
}

#[test]
fn test_issue1948_bare_operation_parameter_ref_is_relative_to_external_path_item() {
    let _g = common::setup(common::V3_MODULE);
    let result = parse("issue-1948/openapi.yaml");
    assert_parameter(Some(parameter(&result, "/products/{param1}")), "param1", "path", "string");
    let response_schema = response_schema(&result, "/products/{param1}").unwrap();
    assert!(result.openapi.as_ref().unwrap().get_components().unwrap().get_schemas().unwrap().get("Product").is_some());
    assert_eq!(response_schema.get_ref().as_deref(), Some("#/components/schemas/Product"));
    assert_no_relative_ref_load_failure(&result);
}

#[test]
fn test_issue2066_dot_slash_path_parameter_ref_is_relative_to_external_path_item() {
    let _g = common::setup(common::V3_MODULE);
    let result = parse("issue-2066/openapi.json");
    let parameter = parameter(&result, "/params");
    assert_eq!(parameter.get_name().as_deref(), Some("limit"));
    assert_eq!(parameter.get_in().as_deref(), Some("query"));
    assert!(parameter.get_schema().is_some());
    assert_eq!(parameter.get_schema().unwrap().get_ref().as_deref(), Some("#/components/schemas/an-int"));
    let resolved = result.openapi.as_ref().unwrap().get_components().unwrap().get_schemas().unwrap().get("an-int").unwrap();
    assert_eq!(resolved.get_type().as_deref(), Some("integer"));
    assert_eq!(resolved.get_format().as_deref(), Some("int32"));
    assert_no_relative_ref_load_failure(&result);
}

#[test]
fn test_uri_illegal_references_are_rebased_end_to_end() {
    let _g = common::setup(common::V3_MODULE);
    let result = parse("issue-2393-regression/openapi.yaml");
    assert_parameter(Some(parameter(&result, "/space")), "space", "query", "string");
    assert_parameter(Some(parameter(&result, "/braces")), "braces", "query", "string");
    assert_parameter(Some(parameter(&result, "/percent")), "percent", "query", "string");
    assert_parameter(Some(parameter(&result, "/prefix-space")), "prefix-space", "query", "string");
    assert_parameter(Some(parameter(&result, "/root-prefix")), "root-prefix", "query", "string");
    let c = result.openapi.as_ref().unwrap().get_components().unwrap();
    assert!(c.get_schemas().unwrap().get("product").is_some());
    assert!(c.get_examples().unwrap().get("example").is_some());
    assert!(c.get_headers().unwrap().get("header").is_some());
    assert!(c.get_links().unwrap().get("link").is_some());
    assert_no_relative_ref_load_failure(&result);
}

#[test]
fn test_resolve_false_does_not_resolve_external_path_item_with_uri_illegal_nested_ref() {
    let _g = common::setup(common::V3_MODULE);
    let mut options = ParseOptions::new();
    options.resolve = false;
    let result = OpenAPIV3Parser::new().read_location("issue-2393-regression/openapi.yaml", None, Some(&options));
    assert!(result.openapi.is_some());
    let path_item = result.openapi.as_ref().unwrap().get_paths().unwrap().get("/space").unwrap();
    assert!(path_item.get_ref().is_some());
    assert!(path_item.get_get().is_none());
    assert_no_relative_ref_load_failure(&result);
}

#[test]
fn test_issue1948_resolve_fully_inlines_external_path_item_parameter() {
    let _g = common::setup(common::V3_MODULE);
    let result = parse_fully("issue-1948/openapi.yaml");
    assert_parameter(Some(parameter(&result, "/products/{param1}")), "param1", "path", "string");
    let response_schema = response_schema(&result, "/products/{param1}");
    assert!(response_schema.is_some());
    assert_eq!(response_schema.unwrap().get_type().as_deref(), Some("object"));
    assert_no_relative_ref_load_failure(&result);
}

#[test]
fn test_issue2066_resolve_fully_inlines_nested_external_path_item_parameter() {
    let _g = common::setup(common::V3_MODULE);
    let result = parse_fully("issue-2066/openapi.json");
    let parameter = parameter(&result, "/params");
    assert_eq!(parameter.get_name().as_deref(), Some("limit"));
    assert_eq!(parameter.get_in().as_deref(), Some("query"));
    let schema = parameter.get_schema().expect("schema");
    assert_eq!(schema.get_type().as_deref(), Some("integer"));
    assert_eq!(schema.get_format().as_deref(), Some("int32"));
    assert_no_relative_ref_load_failure(&result);
}

#[test]
fn test_compute_ref_rebases_bare_relative_ref() {
    let _g = common::setup(common::V3_MODULE);
    for (r, base, expected) in BARE_RELATIVE_REFS_REBASED_BY_COMPUTE_REF {
        assert_eq!(processor().compute_ref(r, base), *expected, "{r} {base}");
    }
}

#[test]
fn test_update_refs_schema_leaves_external_path_item_bare_relative_ref_unchanged() {
    let _g = common::setup(common::V3_MODULE);
    let schema = Schema::new().ref_("schemas/product.yaml");
    processor().update_refs_schema(&schema, "api/path-item.yaml");
    assert_eq!(schema.get_ref().as_deref(), Some("schemas/product.yaml"));
}

#[test]
fn test_update_refs_example_leaves_external_path_item_bare_relative_ref_unchanged() {
    let _g = common::setup(common::V3_MODULE);
    let example = Example::new().ref_("examples/product.yaml");
    processor().update_refs_example(&example, "api/path-item.yaml");
    assert_eq!(example.get_ref().as_deref(), Some("examples/product.yaml"));
}

#[test]
fn test_update_refs_schema_rebases_dot_dot_relative_ref() {
    let _g = common::setup(common::V3_MODULE);
    let schema = Schema::new().ref_("../shared schemas/product.yaml");
    processor().update_refs_schema(&schema, "nested/space-api.yaml");
    assert_eq!(schema.get_ref().as_deref(), Some("shared schemas/product.yaml"));
}

#[test]
fn test_update_refs_example_rebases_dot_dot_relative_ref() {
    let _g = common::setup(common::V3_MODULE);
    let example = Example::new().ref_("../shared examples/example.yaml");
    processor().update_refs_example(&example, "nested/space-api.yaml");
    assert_eq!(example.get_ref().as_deref(), Some("shared examples/example.yaml"));
}

#[test]
fn test_update_refs_composed_schema_all_of_entries_rebase_dot_dot_ref() {
    let _g = common::setup(common::V3_MODULE);
    let composed = Schema::of(SchemaKind::Composed);
    let all_of_entry = Schema::new().ref_("../shared schemas/product.yaml");
    composed.add_all_of_item(all_of_entry.clone());
    processor().update_refs_schema(&composed, "nested/space-api.yaml");
    assert_eq!(all_of_entry.get_ref().as_deref(), Some("shared schemas/product.yaml"));
}

#[test]
fn test_update_refs_composed_schema_any_of_entries_rebase_dot_dot_ref() {
    let _g = common::setup(common::V3_MODULE);
    let composed = Schema::of(SchemaKind::Composed);
    let any_of_entry = Schema::new().ref_("../shared schemas/product.yaml");
    composed.add_any_of_item(any_of_entry.clone());
    processor().update_refs_schema(&composed, "nested/space-api.yaml");
    assert_eq!(any_of_entry.get_ref().as_deref(), Some("shared schemas/product.yaml"));
}

#[test]
fn test_compute_ref_rebases_relative_windows_path_without_normalizing_separators() {
    let _g = common::setup(common::V3_MODULE);
    assert_eq!(processor().compute_ref("schemas\\product.yaml", "api/path-item.yaml"), "api/schemas\\product.yaml");
}

#[test]
fn test_join_with_malformed_remote_base_preserves_dot_segments_unresolved() {
    let _g = common::setup(common::V3_MODULE);
    let result = join(Some("https://example.com/{version}/api.yaml"), Some("../schemas/product.yaml"));
    assert_eq!(result.as_deref(), Some("https://example.com/{version}/../schemas/product.yaml"));
}

#[test]
fn test_bare_relative_schema_ref_in_external_path_item_response_is_resolved() {
    let _g = common::setup(common::V3_MODULE);
    let result = parse("issue-1948/openapi.yaml");
    let response_schema = response_schema(&result, "/products/{param1}").expect("response schema");
    assert_eq!(response_schema.get_ref().as_deref(), Some("#/components/schemas/Product"));
    let product = result.openapi.as_ref().unwrap().get_components().unwrap().get_schemas().unwrap().get("Product").expect("Product");
    assert_eq!(product.get_type().as_deref(), Some("object"));
    assert!(product.get_properties().unwrap().get("productCode").is_some());
    assert_no_relative_ref_load_failure(&result);
}

#[test]
fn test_dot_dot_relative_schema_and_example_refs_in_external_path_item_are_resolved() {
    let _g = common::setup(common::V3_MODULE);
    let result = parse("issue-2393-regression/openapi.yaml");
    let c = result.openapi.as_ref().unwrap().get_components().unwrap();
    let product = c.get_schemas().unwrap().get("product").expect("product");
    assert_eq!(product.get_type().as_deref(), Some("object"));
    assert!(product.get_properties().unwrap().get("id").is_some());
    let example = c.get_examples().unwrap().get("example").expect("example");
    assert_eq!(example.get_summary().as_deref(), Some("Product example"));
    assert_no_relative_ref_load_failure(&result);
}

fn assert_operations_has_parameters(open_api: &OpenAPI, path: &str) {
    let path_item = open_api.get_paths().unwrap().get(path).unwrap();
    assert!(!path_item.read_operations().is_empty(), "Expected operations for {path} but found none");
    for (http_method, operation) in path_item.read_operations_map() {
        assert!(
            !operation.get_parameters().is_none_or(|p| p.is_empty()),
            "Expected parameters on {http_method} operation for {path} but found none"
        );
    }
}

#[test]
fn test_process_paths_parameters_internal_top_level_definition() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPIV3Parser::new().read("src/test/resources/issue-1733/api.yaml").unwrap();
    assert_operations_has_parameters(&open_api, "/internal/test/{id}/toplevelparam");
}

#[test]
fn test_process_paths_parameters_internal_operation_level_definition() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPIV3Parser::new().read("src/test/resources/issue-1733/api.yaml").unwrap();
    assert_operations_has_parameters(&open_api, "/internal/test/{id}/operationlevelparam");
}

#[test]
fn test_process_paths_parameters_ref_top_level_definition() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPIV3Parser::new().read("src/test/resources/issue-1733/api.yaml").unwrap();
    assert_operations_has_parameters(&open_api, "/ref/test/{id}/toplevelparam");
}

#[test]
fn test_process_paths_parameters_ref_operation_level_definition() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPIV3Parser::new().read("src/test/resources/issue-1733/api.yaml").unwrap();
    assert_operations_has_parameters(&open_api, "/ref/test/{id}/operationlevelparam");
}
