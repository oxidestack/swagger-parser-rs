//! Port of `io.swagger.v3.parser.processors.OperationProcessorTest` (`@Mocked ParameterProcessor` and
//! `ResponseProcessor`, injectable cache and OpenAPI).
//!
//! Upstream the test is `@Test(enabled = false)`: with an `@Injectable ResolverCache`, `getParseOptions()`
//! returns a cascaded mock whose `isResolveResponses()` is false, so the recorded `cache.loadRef` of the
//! `$ref` response never happens. Here the cache returns parse options with `resolveResponses` set, which is
//! the configuration under which every recorded expectation applies.

mod common;

use common::mock_cache::{dyn_cache, CacheCall, MockCache};
use common::processor_mocks::{Mocked, ProcCall, Results};
use swagger_parser::core::ParseOptions;
use swagger_parser::java::JList;
use swagger_parser::models::*;
use swagger_parser::parser::processors::OperationProcessor;
use swagger_parser::parser::ref_utils::RefFormat;
use swagger_parser::parser::resolver_cache::ComponentKind;

#[test]
fn test_process_operation() {
    let _g = common::setup(common::V3_MODULE);
    let (cache, open_api) = (MockCache::new(), OpenAPI::new());
    // @Injectable List<Parameter> inputParameterList / outputParameterList, Parameter inputParameter,
    // ApiResponse incomingResponse / resolvedResponse
    let input_parameter = Parameter::new();
    let input_parameter_list = JList::from_vec(vec![input_parameter.clone()]);
    let output_parameter_list = JList::from_vec(vec![Parameter::new()]);
    let incoming_response = ApiResponse::new();
    let resolved_response = ApiResponse::new();

    let m = Mocked::new(&["ParameterProcessor", "ResponseProcessor"], Results { parameters: Some(Some(output_parameter_list.clone())), ..Default::default() });

    let operation = Operation::new();
    operation.set_parameters(input_parameter_list.clone());
    let r = "http://my.company.com/path/to/file.json#/foo/bar";
    let ref_response = ApiResponse::new().ref_(r);
    operation.responses(ApiResponses::new().add_api_response("200", ref_response.clone()));
    operation.get_responses().unwrap().add_api_response("400", incoming_response.clone());

    let mut options = ParseOptions::new();
    options.resolve_responses = true;
    cache.expect_parse_options(options);
    cache.expect_load_ref(r, RefFormat::Url, Some(resolved_response.clone()));

    OperationProcessor::new31(dyn_cache(&cache), &open_api, false).process_operation(&operation).unwrap();

    assert_eq!(m.constructed("ParameterProcessor"), 1);
    assert_eq!(m.constructed("ResponseProcessor"), 1);
    let inv = m.invocations();
    let count = |f: &dyn Fn(&ProcCall) -> bool| inv.iter().filter(|c| f(c)).count();
    assert_eq!(count(&|c| matches!(c, ProcCall::Parameter(p) if p.ptr_eq(&input_parameter))), 1);
    assert_eq!(count(&|c| matches!(c, ProcCall::Parameters(Some(l)) if l.ptr_eq(&input_parameter_list))), 1);
    assert_eq!(count(&|c| matches!(c, ProcCall::Response(x) if x.ptr_eq(&ref_response))), 1);
    assert_eq!(count(&|c| matches!(c, ProcCall::Response(x) if x.ptr_eq(&incoming_response))), 1);
    assert_eq!(count(&|c| matches!(c, ProcCall::Response(x) if x.ptr_eq(&resolved_response))), 1);
    assert_eq!(cache.count(&CacheCall::LoadRef(r.into(), RefFormat::Url, ComponentKind::ApiResponse)), 1);
    // FullVerifications: nothing else on the mocks
    assert_eq!(inv.len(), 5, "{inv:?}");
    assert!(cache.calls().iter().all(|c| matches!(c, CacheCall::LoadRef(..) | CacheCall::GetParseOptions)));

    assert!(operation.get_responses().unwrap().get("200").unwrap().ptr_eq(&resolved_response));
    assert!(operation.get_parameters().unwrap().ptr_eq(&output_parameter_list));
}
