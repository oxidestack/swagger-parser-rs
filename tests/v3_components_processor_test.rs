//! Port of `io.swagger.v3.parser.processors.ComponentsProcessorTest` (JMockit: every processor class the
//! components processor uses is `@Mocked`, the cache is `@Injectable`).

mod common;

use common::mock_cache::{dyn_cache, MockCache};
use common::processor_mocks::{Mocked, ProcCall, Results};
use swagger_parser::models::*;
use swagger_parser::parser::processors::ComponentsProcessor;

const MOCKED: &[&str] = &[
    "SchemaProcessor",
    "ResponseProcessor",
    "RequestBodyProcessor",
    "ParameterProcessor",
    "HeaderProcessor",
    "ExampleProcessor",
    "LinkProcessor",
    "CallbackProcessor",
    "SecuritySchemeProcessor",
];

#[test]
fn test_components_schemas_processor() {
    let _g = common::setup(common::V3_MODULE);
    let mocked = Mocked::new(MOCKED, Results::default());
    let cache = MockCache::new();
    // @Injectable Schema model1, model2
    let model1 = Schema::new();
    let model2 = Schema::new();
    let open_api = OpenAPI::new();
    open_api.components(Components::new().add_schemas("foo", model1.clone()));
    open_api.get_components().unwrap().add_schemas("bar", model2.clone());

    ComponentsProcessor::new31(&open_api, dyn_cache(&cache), false).process_components().unwrap();

    // schemaProcessor.processSchema((Schema) any); times = 2
    let schema_calls = mocked.invocations().iter().filter(|c| matches!(c, ProcCall::Schema(_))).count();
    assert_eq!(schema_calls, 2);
    // Verifications: processSchema(model1), processSchema(model2)
    assert!(mocked.schema_count(&model1) >= 1);
    assert!(mocked.schema_count(&model2) >= 1);
}

#[test]
fn test_no_components_defined() {
    let _g = common::setup(common::V3_MODULE);
    let mocked = Mocked::new(MOCKED, Results::default());
    let cache = MockCache::new();
    // @Injectable OpenAPI openAPI; openAPI.getComponents(); result = null
    let open_api = OpenAPI::new();
    assert!(open_api.get_components().is_none());

    ComponentsProcessor::new31(&open_api, dyn_cache(&cache), false).process_components().unwrap();

    // new XxxProcessor(cache, openAPI[, openapi31]); times = 1
    for class in MOCKED {
        assert_eq!(mocked.constructed(class), 1, "{class}");
    }
    // FullVerifications: nothing else happened on the mocks
    assert!(mocked.invocations().is_empty(), "{:?}", mocked.invocations());
    assert!(cache.calls().is_empty(), "{:?}", cache.calls());
}
