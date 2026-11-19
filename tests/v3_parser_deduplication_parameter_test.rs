//! Port of `io.swagger.v3.parser.test.ParserDeduplicationParameterTest`.

mod common;

use swagger_parser::core::ParseOptions;
use swagger_parser::models::*;
use swagger_parser::parser::OpenAPIV3Parser;

fn read(location: &str, options: &ParseOptions) -> OpenAPI {
    OpenAPIV3Parser::new().read_with(Some(location), None, Some(options)).expect("openAPI")
}

fn params(open_api: &OpenAPI, path: &str) -> Vec<Parameter> {
    open_api.get_paths().unwrap().get(path).unwrap().get_get().unwrap().get_parameters().unwrap().to_vec()
}

fn options(flatten: bool, resolve_combinators: bool, resolve_fully: bool) -> ParseOptions {
    let mut o = ParseOptions::new();
    o.resolve = true;
    if flatten {
        o.flatten = true;
    }
    if resolve_combinators {
        o.resolve_combinators = true;
    }
    if resolve_fully {
        o.resolve_fully = true;
    }
    o
}

#[test]
fn test_issue2102_two_refs_same_name_different_in31() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = read("issue-2102/openapi31.json", &options(false, true, true));
    let params = params(&open_api, "/myoperation");
    assert_eq!(params.len(), 2);
    let param_ins: Vec<Option<String>> = params.iter().map(|p| p.get_in()).collect();
    assert!(param_ins.contains(&Some("query".into())));
    assert!(param_ins.contains(&Some("header".into())));
}

/// Duplicated parameter name with different locations
#[test]
fn test_duplicated_parameter_name_from_ref() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = read("issue-2102/openapi30.json", &options(true, false, false));
    let params = params(&open_api, "/myoperation");
    assert_eq!(params.len(), 2);
    assert_eq!(params.iter().filter(|p| p.get_name().expect("name") == "myParam").count(), 2);
}

/// Duplicated parameter name with different locations
#[test]
fn test_duplicated_parameter_name_from_ref_resolve_fully_resolve_combinators() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = read("issue-2102/openapi30.json", &options(false, true, true));
    let params = params(&open_api, "/myoperation");
    assert_eq!(params.len(), 2);
    assert_eq!(params.iter().filter(|p| p.get_name().expect("name") == "myParam").count(), 2);
}

fn descriptions(params: &[Parameter]) -> Vec<String> {
    params.iter().map(|p| format!("{}:{}", p.get_in().unwrap_or_else(|| "null".into()), p.get_name().unwrap_or_else(|| "null".into()))).collect()
}

#[test]
fn test_inline_and_ref_same_name_different_in() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = read("issue-2102/openapi30mixedRef.yaml", &options(false, false, true));
    let params = params(&open_api, "/mixed");
    assert_eq!(params.len(), 2);
    let d = descriptions(&params);
    assert!(d.contains(&"query:myParam".to_string()));
    assert!(d.contains(&"header:myParam".to_string()));
}

#[test]
fn test_inline_and_ref_same_name_same_in() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = read("issue-2102/openapi30mixedRef.yaml", &options(false, false, true));
    let params = params(&open_api, "/mixed");
    assert_eq!(params.len(), 2);
    let d = descriptions(&params);
    assert!(d.contains(&"query:myParam".to_string()));
    assert!(d.contains(&"header:myParam".to_string()));
}

#[test]
fn test_operation_level_same_name_same_in_ref_overrides_inline() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = read("issue-2102/openapi30sameNameSameLocation.yaml", &options(false, false, true));
    let params = params(&open_api, "/duplicate");
    assert_eq!(params.len(), 1);
    let p = &params[0];
    assert_eq!(p.get_name().as_deref(), Some("myParam"));
    assert_eq!(p.get_in().as_deref(), Some("query"));
    assert_eq!(p.get_schema().unwrap().get_format().as_deref(), Some("date-time"));
}
