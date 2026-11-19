//! Port of `io.swagger.v3.parser.processors.ParameterProcessorTest` (`@Mocked SchemaProcessor`, injectable
//! cache, OpenAPI and parameters: fresh model objects whose getters return null).

mod common;

use common::mock_cache::{dyn_cache, CacheCall, MockCache};
use common::processor_mocks::{Mocked, ProcCall, Results};
use swagger_parser::java::JList;
use swagger_parser::models::*;
use swagger_parser::parser::processors::{ParameterProcessor, RequestBodyProcessor};
use swagger_parser::parser::ref_utils::RefFormat;
use swagger_parser::parser::resolver_cache::ComponentKind;

fn mocked() -> Mocked {
    Mocked::new(&["SchemaProcessor"], Results::default())
}

#[test]
fn test_process_parameters_types_that_are_not_ref_or_body() {
    let _g = common::setup(common::V3_MODULE);
    let (cache, open_api) = (MockCache::new(), OpenAPI::new());
    let m = mocked();
    let header_parameter = Parameter::of(ParameterKind::Header);
    let query_parameter = Parameter::of(ParameterKind::Query);
    let cookie_parameter = Parameter::of(ParameterKind::Cookie);
    let path_parameter = Parameter::of(ParameterKind::Path);
    // getSchema()/getContent(): result = null
    for p in [&header_parameter, &query_parameter, &cookie_parameter, &path_parameter] {
        assert!(p.get_schema().is_none() && p.get_content().is_none() && p.get_ref().is_none());
    }

    let list = JList::from_vec(vec![header_parameter.clone(), query_parameter.clone(), cookie_parameter.clone(), path_parameter.clone()]);
    let processed = ParameterProcessor::new31(dyn_cache(&cache), &open_api, false).process_parameters(Some(&list)).unwrap().unwrap();

    // expectedModelProcessorCreation: new SchemaProcessor(cache, openAPI, openapi31); times = 1
    assert_eq!(m.constructed("SchemaProcessor"), 1);
    // FullVerifications: no schema processing, no cache access
    assert!(m.invocations().is_empty(), "{:?}", m.invocations());
    assert!(cache.calls().is_empty(), "{:?}", cache.calls());

    assert_eq!(processed.len(), 4);
    assert!(processed.get(0).unwrap().ptr_eq(&header_parameter));
    assert!(processed.get(1).unwrap().ptr_eq(&query_parameter));
    assert!(processed.get(2).unwrap().ptr_eq(&cookie_parameter));
    assert!(processed.get(3).unwrap().ptr_eq(&path_parameter));
}

#[test]
fn test_process_parameters_ref_to_header() {
    let _g = common::setup(common::V3_MODULE);
    let (cache, open_api) = (MockCache::new(), OpenAPI::new());
    let m = mocked();
    let r = "#/components/parameters/foo";
    let ref_parameter = Parameter::new().ref_(r);
    let resolved_header_param = Parameter::of(ParameterKind::Header);
    // expectLoadingRefFromCache(ref, INTERNAL, resolvedHeaderParam)
    cache.expect_load_ref(r, RefFormat::Internal, Some(resolved_header_param.clone()));

    let processed = ParameterProcessor::new31(dyn_cache(&cache), &open_api, false)
        .process_parameters(Some(&JList::from_vec(vec![ref_parameter])))
        .unwrap()
        .unwrap();

    assert_eq!(m.constructed("SchemaProcessor"), 1);
    // cache.loadRef(ref, refFormat, Parameter.class); times = 1 — and FullVerifications
    assert_eq!(cache.calls(), vec![CacheCall::LoadRef(r.into(), RefFormat::Internal, ComponentKind::Parameter)]);
    assert!(m.invocations().is_empty(), "{:?}", m.invocations());

    assert_eq!(processed.len(), 1);
    assert!(processed.get(0).unwrap().ptr_eq(&resolved_header_param));
}

#[test]
fn test_process_parameters_body_parameter() {
    let _g = common::setup(common::V3_MODULE);
    let (cache, open_api) = (MockCache::new(), OpenAPI::new());
    let m = mocked();
    let body_param_schema = Schema::new();

    let body_parameter = RequestBody::new().content(Content::new().add_media_type("*/*", MediaType::new().schema(body_param_schema.clone())));
    RequestBodyProcessor::new31(dyn_cache(&cache), &open_api, false).process_request_body(&body_parameter).unwrap();

    // schemaProcessor1[0] = new SchemaProcessor(cache, openAPI, openapi31); times = 1
    assert_eq!(m.constructed("SchemaProcessor"), 1);
    // schemaProcessor1[0].processSchema(bodyParamSchema); times = 1 — and FullVerifications
    assert_eq!(m.schema_count(&body_param_schema), 1);
    assert_eq!(m.invocations().len(), 1);
    assert!(matches!(&m.invocations()[0], ProcCall::Schema(_)));
    assert!(cache.calls().is_empty());
}
