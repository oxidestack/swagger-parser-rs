//! Port of `io.swagger.v3.parser.processors.SchemaProcessorTest` (`@Mocked ExternalRefProcessor`,
//! `@Injectable ResolverCache`/`OpenAPI`).

mod common;

use common::mock_cache::{dyn_cache, MockCache};
use common::processor_mocks::{Mocked, Results};
use swagger_parser::models::*;
use swagger_parser::parser::processors::SchemaProcessor;
use swagger_parser::parser::ref_utils::RefFormat;

/// `@Mocked ExternalRefProcessor` with `processRefToExternalSchema(ref, format); result = ...` recorded
fn mocked(results: &[(&str, RefFormat, &str)]) -> Mocked {
    let external = results.iter().map(|(r, f, n)| ("Schema".to_string(), r.to_string(), *f, Some(n.to_string()))).collect();
    Mocked::new(&["ExternalRefProcessor"], Results { external, ..Default::default() })
}

#[test]
fn test_process_ref_schema_external_ref() {
    let _g = common::setup(common::V3_MODULE);
    let (cache, open_api) = (MockCache::new(), OpenAPI::new());
    let r = "http://my.company.com/path/to/file.json#/foo/bar";
    let m = mocked(&[(r, RefFormat::Url, "bar")]);

    let ref_schema = Schema::new().ref_(r);
    SchemaProcessor::new(dyn_cache(&cache), &open_api).process_schema(Some(&ref_schema)).unwrap();

    assert_eq!(m.external_count("Schema", r, RefFormat::Url), 1);
    assert_eq!(ref_schema.get_ref().as_deref(), Some("#/components/schemas/bar"));
}

#[test]
fn test_process_ref_schema_internal_ref() {
    let _g = common::setup(common::V3_MODULE);
    let (cache, open_api) = (MockCache::new(), OpenAPI::new());
    let _m = mocked(&[]);
    let r = "#/components/schemas/bar";
    let ref_model = Schema::new().ref_(r);
    SchemaProcessor::new(dyn_cache(&cache), &open_api).process_schema(Some(&ref_model)).unwrap();
    assert_eq!(ref_model.get_ref().as_deref(), Some(r));
}

#[test]
fn test_process_array_schema() {
    let _g = common::setup(common::V3_MODULE);
    let (cache, open_api) = (MockCache::new(), OpenAPI::new());
    let _m = mocked(&[]);
    let property = Schema::new();
    let property_processor = SchemaProcessor::new(dyn_cache(&cache), &open_api);
    property_processor.process_schema(Some(&property)).unwrap();

    let model = Schema::of(SchemaKind::Array);
    model.set_items(property.clone());
    let schema_processor = SchemaProcessor::new(dyn_cache(&cache), &open_api);
    schema_processor.process_schema(Some(&model)).unwrap();

    assert_eq!(Some(property), model.get_items());
}

#[test]
fn test_process_composed_schema() {
    let _g = common::setup(common::V3_MODULE);
    let (cache, open_api) = (MockCache::new(), OpenAPI::new());
    let ref1 = "http://my.company.com/path/to/file.json#/foo/bar";
    let ref2 = "http://my.company.com/path/to/file.json#/this/that";
    let ref3 = "http://my.company.com/path/to/file.json#/hello/world";
    let m = mocked(&[(ref1, RefFormat::Url, "bar"), (ref2, RefFormat::Url, "that"), (ref3, RefFormat::Url, "world")]);
    let model_processor = SchemaProcessor::new(dyn_cache(&cache), &open_api);

    let composed = Schema::of(SchemaKind::Composed);
    composed.add_all_of_item(Schema::new().ref_(ref1));
    composed.add_all_of_item(Schema::new().ref_(ref2));
    composed.add_all_of_item(Schema::new().ref_(ref3));
    model_processor.process_schema(Some(&composed)).unwrap();

    // FullVerifications: exactly these three invocations, once each
    assert_eq!(m.external_count("Schema", ref1, RefFormat::Url), 1);
    assert_eq!(m.external_count("Schema", ref2, RefFormat::Url), 1);
    assert_eq!(m.external_count("Schema", ref3, RefFormat::Url), 1);
    assert_eq!(m.invocations().len(), 3);

    let all_of = composed.get_all_of().unwrap();
    assert_eq!(all_of.get(0).unwrap().get_ref().as_deref(), Some("#/components/schemas/bar"));
    assert_eq!(all_of.get(1).unwrap().get_ref().as_deref(), Some("#/components/schemas/that"));
    assert_eq!(all_of.get(2).unwrap().get_ref().as_deref(), Some("#/components/schemas/world"));
}

#[test]
fn test_process_schema() {
    let _g = common::setup(common::V3_MODULE);
    let (cache, open_api) = (MockCache::new(), OpenAPI::new());
    let _m = mocked(&[]);
    let property1 = Schema::new();
    let property2 = Schema::new();
    let property_processor = SchemaProcessor::new(dyn_cache(&cache), &open_api);

    let model = Schema::new();
    model.add_properties("foo", property1.clone());
    model.add_properties("bar", property2.clone());

    property_processor.process_schema(Some(&property1)).unwrap();
    property_processor.process_schema(Some(&property2)).unwrap();

    SchemaProcessor::new(dyn_cache(&cache), &open_api).process_schema(Some(&model)).unwrap();

    assert_eq!(model.get_properties().unwrap().get("foo"), Some(property1));
    assert_eq!(model.get_properties().unwrap().get("bar"), Some(property2));
}

#[test]
fn test_process_ref_property_external_ref() {
    let _g = common::setup(common::V3_MODULE);
    let (cache, open_api) = (MockCache::new(), OpenAPI::new());
    let r = "http://my.company.com/path/to/file.json#/foo/bar";
    let m = mocked(&[(r, RefFormat::Url, "bar")]);
    let ref_property = Schema::new().ref_(r);

    SchemaProcessor::new(dyn_cache(&cache), &open_api).process_schema(Some(&ref_property)).unwrap();

    // new ExternalRefProcessor(cache, openAPI); times = 1
    assert_eq!(m.constructed("ExternalRefProcessor"), 1);
    // FullVerifications
    assert_eq!(m.external_count("Schema", r, RefFormat::Url), 1);
    assert_eq!(m.invocations().len(), 1);
    assert_eq!(ref_property.get_ref().as_deref(), Some("#/components/schemas/bar"));
}

#[test]
fn test_process_ref_property_internal_ref() {
    let _g = common::setup(common::V3_MODULE);
    let (cache, open_api) = (MockCache::new(), OpenAPI::new());
    let m = mocked(&[]);
    let expected_ref = "#/components/schemas/foo";
    let property = Schema::new().ref_(expected_ref);
    SchemaProcessor::new(dyn_cache(&cache), &open_api).process_schema(Some(&property)).unwrap();

    assert_eq!(m.constructed("ExternalRefProcessor"), 1);
    // FullVerifications {{ }}
    assert!(m.invocations().is_empty());
    assert!(cache.calls().is_empty());
    assert_eq!(property.get_ref().as_deref(), Some(expected_ref));
}

#[test]
fn test_process_array_property_items_is_ref_property() {
    let _g = common::setup(common::V3_MODULE);
    let (cache, open_api) = (MockCache::new(), OpenAPI::new());
    let r = "http://my.company.com/path/to/file.json#/foo/bar";
    let m = mocked(&[(r, RefFormat::Url, "bar")]);
    let ref_property = Schema::new().ref_(r);
    let array_property = Schema::of(SchemaKind::Array);
    array_property.set_items(ref_property);

    SchemaProcessor::new(dyn_cache(&cache), &open_api).process_schema(Some(&array_property)).unwrap();

    assert_eq!(m.constructed("ExternalRefProcessor"), 1);
    assert_eq!(m.external_count("Schema", r, RefFormat::Url), 1);
    assert_eq!(m.invocations().len(), 1);
    assert_eq!(array_property.get_items().unwrap().get_ref().as_deref(), Some("#/components/schemas/bar"));
}

#[test]
fn test_process_map_property_additional_properties_is_ref_property() {
    let _g = common::setup(common::V3_MODULE);
    let (cache, open_api) = (MockCache::new(), OpenAPI::new());
    let r = "http://my.company.com/path/to/file.json#/foo/bar";
    let m = mocked(&[(r, RefFormat::Url, "bar")]);
    let ref_property = Schema::new().ref_(r);
    ref_property.set_additional_properties(AdditionalProperties::Schema(ref_property.clone()));

    SchemaProcessor::new(dyn_cache(&cache), &open_api).process_schema(Some(&ref_property)).unwrap();

    assert_eq!(m.constructed("ExternalRefProcessor"), 1);
    assert_eq!(m.external_count("Schema", r, RefFormat::Url), 1);
    assert_eq!(m.invocations().len(), 1);
    let Some(AdditionalProperties::Schema(ap)) = ref_property.get_additional_properties() else { panic!("additionalProperties") };
    assert_eq!(ap.get_ref().as_deref(), Some("#/components/schemas/bar"));
    // break the self-reference
    ref_property.set_additional_properties(None::<AdditionalProperties>);
}
