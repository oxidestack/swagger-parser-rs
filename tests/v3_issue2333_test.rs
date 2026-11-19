//! Port of `io.swagger.v3.parser.test.Issue2333Test`.

mod common;

use swagger_parser::core::ParseOptions;
use swagger_parser::java::JMap;
use swagger_parser::models::*;
use swagger_parser::parser::OpenAPIV3Parser;

fn parse(fixture: &str) -> JMap<Schema> {
    let mut options = ParseOptions::new();
    options.resolve = true;
    options.resolve_responses = true;
    let result = OpenAPIV3Parser::new().read_location(&format!("./src/test/resources/issue-2333/{fixture}"), None, Some(&options));
    assert!(result.openapi.is_some());
    let messages = result.messages.clone().unwrap_or_default();
    assert!(messages.is_empty(), "Unexpected parser messages: {}", common::java_list(&messages));
    let components = result.openapi.unwrap().get_components().expect("components");
    components.get_schemas().expect("schemas")
}

fn assert_schema_has_only_property(schema: Option<Schema>, property: &str) {
    let schema = schema.expect("schema");
    let props = schema.get_properties().expect("properties");
    assert_eq!(props.len(), 1);
    assert!(props.contains_key(property), "Expected property {property}, but was {}", common::java_list(&props.keys()));
}

fn resolve(schemas: &JMap<Schema>, schema: Option<Schema>) -> Option<Schema> {
    if let Some(r) = schema.as_ref().and_then(|s| s.get_ref()) {
        let name = &r[r.rfind('/').map(|i| i + 1).unwrap_or(0)..];
        return schemas.get(name);
    }
    schema
}

fn same(a: Option<Schema>, b: Option<Schema>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a.ptr_eq(&b),
        (None, None) => true,
        _ => false,
    }
}

fn contains_all(schemas: &JMap<Schema>, names: &[&str]) -> bool {
    names.iter().all(|n| schemas.contains_key(n))
}

/// Two schemas defined in different files but sharing the same name (both `Pet`) must be resolved as distinct
/// components.
#[test]
fn referenced_models_with_same_name_from_different_files_are_not_merged() {
    let _g = common::setup(common::V3_MODULE);
    let schemas = parse("main.yaml");

    assert_eq!(schemas.len(), 3);
    assert!(contains_all(&schemas, &["SomeItem", "Pet", "Pet_1"]));
    assert_schema_has_only_property(resolve(&schemas, schemas.get("Pet")), "id");
    assert_schema_has_only_property(schemas.get("Pet_1"), "name");
    assert!(same(schemas.get("SomeItem"), schemas.get("Pet_1")));
    assert_schema_has_only_property(resolve(&schemas, schemas.get("SomeItem")), "name");
}

#[test]
fn identical_models_with_same_name_from_different_files_reuse_single_component() {
    let _g = common::setup(common::V3_MODULE);
    let schemas = parse("identical-models-different-files.yaml");

    assert_eq!(schemas.len(), 3);
    assert!(contains_all(&schemas, &["FirstPet", "SecondPet", "Pet"]));
    assert!(!schemas.contains_key("Pet_1"));
    assert!(same(schemas.get("FirstPet"), schemas.get("Pet")));
    assert!(same(schemas.get("SecondPet"), schemas.get("Pet")));
    assert_schema_has_only_property(resolve(&schemas, schemas.get("FirstPet")), "id");
    assert_schema_has_only_property(resolve(&schemas, schemas.get("SecondPet")), "id");
}

#[test]
fn equivalent_refs_with_different_spelling_reuse_the_placeholder_name() {
    let _g = common::setup(common::V3_MODULE);
    let schemas = parse("equivalent-refs.yaml");

    assert_eq!(schemas.len(), 2);
    assert!(schemas.contains_key("Thing"));
    assert!(!schemas.contains_key("Thing_1"));
    assert!(same(schemas.get("ThingAlias"), schemas.get("Thing")));
    assert_schema_has_only_property(resolve(&schemas, schemas.get("ThingAlias")), "value");
}

#[test]
fn reversed_traversal_preserves_distinct_targets_and_collapses_equivalent_refs() {
    let _g = common::setup(common::V3_MODULE);
    let schemas = parse("reversed.yaml");

    assert_eq!(schemas.len(), 4);
    assert!(contains_all(&schemas, &["Pet", "SomeItem", "PetAlias", "Pet_1"]));
    assert!(!schemas.contains_key("Pet_2"));
    assert_schema_has_only_property(resolve(&schemas, schemas.get("Pet")), "id");
    assert_schema_has_only_property(schemas.get("Pet_1"), "name");
    assert!(same(schemas.get("SomeItem"), schemas.get("Pet_1")));
    assert!(same(schemas.get("PetAlias"), schemas.get("Pet")));
    assert_schema_has_only_property(resolve(&schemas, schemas.get("SomeItem")), "name");
    assert_schema_has_only_property(resolve(&schemas, schemas.get("PetAlias")), "id");
}

#[test]
fn case_insensitive_name_collision_preserves_distinct_targets() {
    let _g = common::setup(common::V3_MODULE);
    let schemas = parse("case-insensitive.yaml");

    assert_eq!(schemas.len(), 3);
    assert!(contains_all(&schemas, &["SomeItem", "pet", "Pet_1"]));
    assert!(!schemas.contains_key("Pet"));
    assert_schema_has_only_property(resolve(&schemas, schemas.get("pet")), "id");
    assert_schema_has_only_property(schemas.get("Pet_1"), "name");
    assert!(same(schemas.get("SomeItem"), schemas.get("Pet_1")));
    assert_schema_has_only_property(resolve(&schemas, schemas.get("SomeItem")), "name");
}
