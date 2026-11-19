//! Port of `io.swagger.v3.parser.test.OAI31DeserializationTest`.

mod common;

use jackson::{BigDecimal, JavaObject, JsonNode};
use swagger_parser::core::{ParseOptions, SwaggerParseResult};
use swagger_parser::models::Schema;
use swagger_parser::parser::OpenAPIV3Parser;

fn msgs(r: &SwaggerParseResult) -> Vec<String> {
    r.messages.clone().expect("messages")
}

fn read_location(loc: &str, options: Option<&ParseOptions>) -> SwaggerParseResult {
    OpenAPIV3Parser::new().read_location(loc, None, options)
}

fn read_contents(s: &str, options: Option<&ParseOptions>) -> SwaggerParseResult {
    OpenAPIV3Parser::new().read_contents(s, None, options)
}

fn obj(fields: Vec<(&str, JsonNode)>) -> JsonNode {
    JsonNode::object(fields.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

fn node(n: JsonNode) -> JavaObject {
    JavaObject::Node(n)
}

#[test]
fn test_schema_keys_oas31() {
    let _g = common::setup(common::V3_MODULE);
    let result = read_location("3.1.0/petstore-3.1_more.yaml", None);
    let open_api = result.openapi.clone().unwrap();
    assert!(msgs(&result).is_empty());
    let s = open_api.get_components().unwrap().get_schemas().unwrap().get("Schema2020_12").unwrap();
    assert_eq!(s.get_const(), Some(JavaObject::string("const text")));
    assert_eq!(s.get_id().as_deref(), Some("schemaId"));
    assert_eq!(s.get_comment().as_deref(), Some("comment for testing"));
    assert!(s.get_if().unwrap().get_properties().unwrap().get("country").is_some());
    assert!(s.get_then().unwrap().get_properties().unwrap().get("maple_trees").is_some());
    assert!(s.get_else().unwrap().get_properties().unwrap().get("accept").is_some());
    let unevaluated_property = s.get_unevaluated_properties().expect("instanceof Schema");
    assert!(unevaluated_property.get_types().unwrap().contains("object"));
    assert_eq!(s.get_content_media_type().as_deref(), Some("text/html"));
    assert_eq!(s.get_anchor().as_deref(), Some("anchor text"));
    assert_eq!(s.get_schema().as_deref(), Some("https://json-schema.org/draft/2020-12/schema"));
    assert!(s.get_examples().unwrap().get(0).is_some_and(|e| !e.is_null()));
    assert_eq!(s.get_content_encoding().as_deref(), Some("base64"));
    assert_eq!(s.get_min_contains(), Some(2));
    assert_eq!(s.get_max_contains(), Some(4));
    let prefix_items = s.get_prefix_items().unwrap().get(0).expect("instanceof Schema");
    assert_eq!(prefix_items.get_description().as_deref(), Some("Name"));
    assert!(s.get_contains().unwrap().get_types().unwrap().contains("integer"));
    assert!(s.get_content_schema().unwrap().get_types().unwrap().contains("string"));
    assert_eq!(s.get_property_names().unwrap().get_pattern().as_deref(), Some("^[A-Za-z_][A-Za-z0-9_]*$"));
    let dependant_schema = s.get_dependent_schemas().unwrap().get("credit_card").expect("instanceof Schema");
    assert_eq!(dependant_schema.get_required().unwrap().get(0).as_deref(), Some("billing_address"));
    assert!(s.get_dependent_required().unwrap().contains_key("credit_card"));
    let pattern_properties = s.get_pattern_properties().unwrap().get("^S_").expect("instanceof Schema");
    assert!(s.get_unevaluated_items().unwrap().get_types().unwrap().contains("object"));
    assert!(pattern_properties.get_types().unwrap().contains("string"));
}

#[test]
fn test_schema_const_oas31() {
    let _g = common::setup(common::V3_MODULE);
    let result = read_location("3.1.0/issue-1975.yaml", None);
    let open_api = result.openapi.clone().unwrap();
    assert!(msgs(&result).is_empty());
    let schemas = open_api.get_components().unwrap().get_schemas().unwrap();
    let c = |name: &str| schemas.get(name).unwrap().get_const();
    assert_eq!(c("ConstValidation"), Some(JavaObject::Integer(2)));
    let obj_node = obj(vec![("foo", JsonNode::text("bar")), ("baz", JsonNode::text("bax"))]);
    assert_eq!(c("ConstWithObject"), Some(node(obj_node)));
    let array_node = JsonNode::array(vec![obj(vec![("foo", JsonNode::text("bar"))])]);
    assert_eq!(c("ConstWithArray"), Some(node(array_node)));
    assert_eq!(c("ConstWithNull"), Some(node(JsonNode::Null)));
    assert_eq!(c("ConstWithFalseDoesNotMatch0"), Some(JavaObject::Boolean(false)));
    assert_eq!(c("ConstWithTrueDoesNotMatch1"), Some(JavaObject::Boolean(true)));
    assert_eq!(c("ConstWithArrayFalseDoesNotMatch0"), Some(node(JsonNode::array(vec![JsonNode::Boolean(false)]))));
    assert_eq!(c("ConstWithArrayTrueDoesNotMatch1"), Some(node(JsonNode::array(vec![JsonNode::Boolean(true)]))));
    assert_eq!(c("ConstWithAFalseDoesNotMatchA0"), Some(node(obj(vec![("a", JsonNode::Boolean(false))]))));
    assert_eq!(c("ConstWithATrueDoesNotMatchA1"), Some(node(obj(vec![("a", JsonNode::Boolean(true))]))));
    assert_eq!(c("ConstWith0DoesNotMatchOtherZeroLikeTypes"), Some(JavaObject::Integer(0)));
    assert_eq!(c("ConstWith1DoesNotMatchTrue"), Some(JavaObject::Integer(1)));
    assert_eq!(c("ConstWith20MatchesIntegerAndFloatTypes"), Some(JavaObject::BigDecimal(BigDecimal::parse("-2.0").unwrap().into())));
    assert_eq!(
        c("ConstFloatAndIntegersAreEqualUpTo64BitRepresentationLimits"),
        Some(JavaObject::BigDecimal(BigDecimal::from_i64(9007199254740992).into()))
    );
    assert_eq!(c("ConstNulCharactersInStrings"), Some(JavaObject::string("hello\0there")));
}

#[test]
fn test_basic_oas31() {
    let _g = common::setup(common::V3_MODULE);
    let result = read_location("3.1.0/test/basicOAS31.yaml", None);
    let open_api = result.openapi.clone().unwrap();
    let m = msgs(&result);
    assert_eq!(open_api.get_json_schema_dialect().as_deref(), Some("https://json-schema.org/draft/2020-12/schema"));
    let info = open_api.get_info().unwrap();
    assert_eq!(info.get_summary().as_deref(), Some("test summary in info object"));
    assert_eq!(info.get_description().as_deref(), Some("description in info object"));
    assert_eq!(info.get_license().unwrap().get_identifier().as_deref(), Some("test identifier"));
    let components = open_api.get_components().unwrap();
    assert!(components.get_path_items().unwrap().get("pets").is_some());
    let schemas = components.get_schemas().unwrap();
    let s = |n: &str| schemas.get(n).unwrap();
    assert!(s("ArrayWithoutItems").get_types().unwrap().contains("array"));
    assert!(s("ArrayWithoutItems").get_items().is_none());
    assert!(!m.contains(&"attribute components.schemas.ArrayWithoutItems.items is missing".to_string()));
    assert!(s("ItemsWithoutArrayType").get_types().unwrap().contains("object"));
    assert!(s("ItemsWithoutArrayType").get_items().is_some());
    assert!(!m.contains(&"attribute components.schemas.ItemsWithoutArrayType.item1 is unexpected".to_string()));
    let pet_types = s("Pet").get_types().unwrap();
    assert_eq!(pet_types.len(), 3);
    assert!(pet_types.contains("array"));
    assert!(pet_types.contains("string"));
    assert!(pet_types.contains("object"));
    assert!(s("Pet").get_extensions().unwrap().get("arbitraryKeyword").is_some());
    assert!(!m.contains(&"attribute components.schemas.Pet.arbitraryKeyword is unexpected".to_string()));
    assert!(s("Pet").get_properties().unwrap().get("testconst").unwrap().get_const().is_some());
    assert!(!m.contains(&"attribute components.schemas.Pet.const is unexpected".to_string()));
    assert_eq!(s("Pets").get_exclusive_maximum_value().unwrap().to_i32_wrapping(), 12);
    assert_eq!(s("Pets").get_exclusive_minimum_value().unwrap().to_i32_wrapping(), 1);
    assert!(s("Pets").get_types().unwrap().contains("null"));
    assert_eq!(s("Pets").get_default(), Some(JavaObject::string("I'm a string")));
    assert!(s("MapAnyValue").get_types().is_none());
    assert!(s("Pet").get_ref().is_some());
    assert!(s("Pet").get_properties().is_some());
}

#[test]
fn test_basic_oas30_with31_fields() {
    let _g = common::setup(common::V3_MODULE);
    let result = read_location("3.1.0/test/basicOAS30.yaml", None);
    let open_api = result.openapi.clone().unwrap();
    let m = msgs(&result);
    let has = |s: &str| m.iter().any(|x| x == s);
    assert!(open_api.get_json_schema_dialect().is_none());
    assert!(has("attribute jsonSchemaDialect is unexpected"));
    let info = open_api.get_info().unwrap();
    assert_eq!(info.get_description().as_deref(), Some("description in info object"));
    assert!(has("attribute info.summary is unexpected"));
    assert!(info.get_license().unwrap().get_identifier().is_none());
    assert!(has("attribute info.license.identifier is unexpected"));
    let components = open_api.get_components().unwrap();
    assert!(components.get_path_items().is_none());
    assert!(has("attribute components.pathItems is unexpected"));
    let schemas = components.get_schemas().unwrap();
    let s = |n: &str| schemas.get(n).unwrap();
    assert_eq!(s("ArrayWithoutItems").get_type().as_deref(), Some("array"));
    assert!(s("ArrayWithoutItems").get_items().is_none());
    assert!(has("attribute components.schemas.ArrayWithoutItems.items is missing"));
    assert_ne!(s("ItemsWithoutArrayType").get_type().as_deref(), Some("object"));
    assert_eq!(s("ItemsWithoutArrayType").get_type().as_deref(), Some("array"));
    assert!(s("ItemsWithoutArrayType").get_items().is_some());
    assert!(has("attribute components.schemas.ItemsWithoutArrayType.item1 is unexpected"));
    assert!(s("Pet").get_types().is_none());
    assert!(s("Pet").get_extensions().is_none());
    assert!(has("attribute components.schemas.Pet.arbitraryKeyword is unexpected"));
    assert!(s("Pet").get_properties().unwrap().get("testconst").unwrap().get_const().is_none());
    assert!(has("attribute components.schemas.Pet.const is unexpected"));
    assert!(s("Pets").get_exclusive_maximum().is_none());
    assert!(s("Pets").get_exclusive_minimum().is_none());
    assert_eq!(s("Pets").get_types().unwrap().items()[0], "array");
    assert!(s("Pets").get_default().is_none());
    assert_eq!(s("MapAnyValue").get_types().unwrap().items()[0], "object");
    assert!(has("attribute webhooks is unexpected"));
}

#[test]
fn test_deserialize_simple_definition() {
    let _g = common::setup(common::V3_MODULE);
    let json = "{\n  \"openapi\": \"3.1.0\",\n  \"info\": {\n    \"title\": \"Swagger Petstore\",\n    \"description\": \"This is a sample server Petstore server. You can find out more about Swagger at [http://swagger.io](http://swagger.io) or on [irc.freenode.net, #swagger](http://swagger.io/irc/). For this sample, you can use the api key `special-key` to test the authorization filters.\",\n    \"termsOfService\": \"http://swagger.io/terms/\",\n    \"contact\": {\n      \"email\": \"apiteam@swagger.io\"\n    },\n    \"license\": {\n      \"name\": \"Apache 2.0\",\n      \"url\": \"http://www.apache.org/licenses/LICENSE-2.0.html\"\n    },\n    \"version\": \"1.0.0\"\n  }\n}";
    let options = ParseOptions { resolve: true, ..Default::default() };
    let result = read_contents(json, Some(&options));
    assert!(result.openapi.is_some());
}

#[test]
fn test_basic() {
    let _g = common::setup(common::V3_MODULE);
    let result = read_location("3.1.0/basic.yaml", None);
    assert!(result.openapi.is_some());
}

#[test]
fn test_oas31() {
    let _g = common::setup(common::V3_MODULE);
    let result = read_location("3.1.0/oas3.1.yaml", None);
    assert!(result.openapi.is_some());
}

#[test]
fn test_json_schema_dialect_valid() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = "openapi: 3.1.0\njsonSchemaDialect: https://json-schema.org/draft/2020-12/schema\ninfo:\n  title: Swagger Petstore\n  version: 1.0.0\npaths: {}";
    let result = read_contents(yaml, None);
    let open_api = result.openapi.unwrap();
    assert_eq!(open_api.get_json_schema_dialect().as_deref(), Some("https://json-schema.org/draft/2020-12/schema"));
}

#[test]
fn test_json_schema_dialect_invalid() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = "openapi: 3.1.0\njsonSchemaDialect: bad URI\ninfo:\n  title: Swagger Petstore\n  version: 1.0.0\npaths: {}";
    let result = read_contents(yaml, None);
    assert!(result.openapi.is_some());
    assert!(msgs(&result).contains(&"jsonSchemaDialect. Invalid url: bad URI".to_string()));
}

const INFO_YAML: &str = "openapi: 3.1.0\ninfo:\n  title: Swagger Petstore\n  summary: test summary in info object\n  description: \"This is a sample server Petstore server. You can find out more about\\\n    \\ Swagger at [http://swagger.io](http://swagger.io) or on [irc.freenode.net, #swagger](http://swagger.io/irc/).\\\n    \\ For this sample, you can use the api key `special-key` to test the authorization\\\n    \\ filters.\"\n  termsOfService: http://swagger.io/terms/\n  contact:\n    email: apiteam@swagger.io\n  license:\n    name: Apache 2.0\n    url: http://www.apache.org/licenses/LICENSE-2.0.html\n  version: 1.0.0\nservers:\n- url: /\n";

#[test]
fn test_info() {
    let _g = common::setup(common::V3_MODULE);
    let result = read_contents(INFO_YAML, None);
    let open_api = result.openapi.clone().unwrap();
    assert!(open_api.get_info().unwrap().get_summary().is_some());
    assert!(!msgs(&result).contains(&"attribute info.summary is unexpected".to_string()));
}

#[test]
fn test_paths_items_under_components() {
    let _g = common::setup(common::V3_MODULE);
    let result = read_location("3.1.0/petstore-3.1_more.yaml", None);
    assert!(result.openapi.as_ref().unwrap().get_components().unwrap().get_path_items().is_some());
    assert!(!msgs(&result).contains(&"attribute components.pathItems is unexpected".to_string()));
}

#[test]
fn test_discriminator_extensions() {
    let _g = common::setup(common::V3_MODULE);
    let result = read_location("3.1.0/petstore-3.1_more.yaml", None);
    let open_api = result.openapi.unwrap();
    let d = open_api.get_components().unwrap().get_schemas().unwrap().get("DiscriminatorExtension").unwrap().get_discriminator().unwrap();
    assert!(d.get_extensions().unwrap().get("x-extension").is_some());
}

#[test]
fn test_security_deserialization() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = "openapi: 3.1.0\nsecurity:\n  - mutualTLS: []\n";
    let result = read_contents(yaml, None);
    let open_api = result.openapi.unwrap();
    assert_eq!(open_api.get_security().unwrap().len(), 1);
}

#[test]
fn test_security_deserialization2() {
    let _g = common::setup(common::V3_MODULE);
    let result = read_location("3.1.0/securitySchemes31.yaml", None);
    let open_api = result.openapi.unwrap();
    assert!(open_api.get_components().unwrap().get_security_schemes().unwrap().get("mutual_TLS").is_some());
}

#[test]
fn test_optional_paths_object() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = "openapi: 3.1.0\ninfo:\n  title: Swagger Petstore\n  version: 1.0.0\n";
    let result = read_contents(yaml, None);
    assert!(result.openapi.is_some());
    assert!(!msgs(&result).contains(&"attribute paths is missing".to_string()));
}

#[test]
fn test_valid_open_api_document() {
    let _g = common::setup(common::V3_MODULE);
    let result = read_contents(INFO_YAML, None);
    assert!(result.openapi.is_some());
    let m = msgs(&result);
    assert!(!m.contains(&"attribute paths is missing".to_string()));
    assert!(m.contains(&"The OpenAPI document MUST contain at least one paths field, a components field or a webhooks field".to_string()));
}

#[test]
fn test_reserved_extensions_oai_author_false() {
    let _g = common::setup(common::V3_MODULE);
    let result = read_location("3.1.0/basic.yaml", None);
    assert!(result.openapi.is_some());
    let m = msgs(&result);
    assert!(m.contains(&"attribute x-oas-internal is reserved by The OpenAPI Initiative".to_string()));
    assert!(m.contains(&"attribute x-oai-extension is reserved by The OpenAPI Initiative".to_string()));
}

#[test]
fn test_reserved_extensions_oai_author_true() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { oai_author: true, ..Default::default() };
    let result = read_location("3.1.0/basic.yaml", Some(&options));
    assert!(result.openapi.is_some());
    let m = msgs(&result);
    assert!(!m.contains(&"attribute x-oas-internal is reserved by The OpenAPI Initiative".to_string()));
    assert!(!m.contains(&"attribute x-oai-extension is reserved by The OpenAPI Initiative".to_string()));
}

#[test]
fn test_siblings_reference_objects() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { oai_author: true, ..Default::default() };
    let result = read_location("3.1.0/siblings31.yaml", Some(&options));
    let open_api = result.openapi.unwrap();
    let paths = open_api.get_paths().unwrap();
    let pets = paths.get("/pets").unwrap();
    assert!(pets.get_ref().is_some() && pets.get_description().is_some() && pets.get_summary().is_some());
    let get = paths.get("/pets/{petId}").unwrap().get_get().unwrap();
    let p0 = get.get_parameters().unwrap().get(0).unwrap();
    assert!(p0.get_ref().is_some() && p0.get_description().is_some());
    let r200 = get.get_responses().unwrap().get("200").unwrap();
    assert!(r200.get_ref().is_some() && r200.get_description().is_some());
    let post = paths.get("/pets/requestBody").unwrap().get_post().unwrap();
    let rb = post.get_request_body().unwrap();
    assert!(rb.get_ref().is_some() && rb.get_description().is_some());
    let resp = post.get_responses().unwrap().get("200").unwrap();
    let h = resp.get_headers().unwrap().get("X-Rate-Limit").unwrap();
    assert!(h.get_ref().is_some() && h.get_description().is_some());
    let l = resp.get_links().unwrap().get("userRepository").unwrap();
    assert!(l.get_ref().is_some() && l.get_description().is_some());
    let ss = open_api.get_components().unwrap().get_security_schemes().unwrap().get("api_key").unwrap();
    assert!(ss.get_ref().is_some() && ss.get_description().is_some());
}

fn component(result: &SwaggerParseResult, name: &str) -> Schema {
    result.openapi.as_ref().unwrap().get_components().unwrap().get_schemas().unwrap().get(name).unwrap()
}

#[test]
fn test_siblings_reference_json_schema1() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions::new();
    let yaml = "openapi: 3.1.0\ninfo:\n  title: siblings JSONSchema\n  version: 1.0.0\nservers:\n  - url: /\npaths: { }\ncomponents:\n  schemas:\n    profile:\n      description: siblings refs\n      required:\n        - login\n        - password\n      maxItems: 2\n      $ref: ./ex.json#user-profile\n      properties:\n        login:\n          type: string\n        password:\n          type: string";
    let result = read_contents(yaml, Some(&options));
    assert!(result.openapi.is_some());
    let profile = component(&result, "profile");
    assert!(profile.get_ref().is_some());
    assert_eq!(profile.get_max_items(), Some(2));
    assert_eq!(profile.get_description().as_deref(), Some("siblings refs"));
    assert_eq!(profile.get_required().unwrap().len(), 2);
    assert!(profile.get_properties().unwrap().contains_key("login"));
    assert!(profile.get_properties().unwrap().contains_key("password"));
}

#[test]
fn test_siblings_reference_json_schema2() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions::new();
    let yaml = "openapi: 3.1.0\ninfo:\n  title: siblings JSONSchema\n  version: 1.0.0\nservers:\n  - url: /\npaths: { }\ncomponents:\n  schemas:\n    profile:\n      $ref: ./ex.json#user-profile\n      pattern: \\d\\d\\d\\d-\\d\\d-\\d\\d\n      patternProperties:\n        \"^S_\":\n          type: string\n        \"^I_\":\n          type: integer\n      additionalProperties: false";
    let result = read_contents(yaml, Some(&options));
    assert!(result.openapi.is_some());
    let profile = component(&result, "profile");
    assert!(profile.get_ref().is_some());
    assert_eq!(profile.get_pattern().as_deref(), Some("\\d\\d\\d\\d-\\d\\d-\\d\\d"));
    assert!(profile.get_additional_properties().is_some());
    assert!(profile.get_pattern_properties().unwrap().contains_key("^S_"));
}

#[test]
fn test_siblings_reference_json_schema3() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions::new();
    let yaml = "openapi: 3.1.0\ninfo:\n  title: siblings JSONSchema\n  version: 1.0.0\nservers:\n  - url: /\npaths: { }\ncomponents:\n  schemas:\n    profile:\n      $id: profile-id\n      $anchor: foo\n      $schema: https://json-schema.org/draft/2020-12/schema\n      $comment: end user should not see this comment\n      type:\n        - string\n        - integer\n      exclusiveMaximum: 12\n      exclusiveMinimum: 1\n      $ref: ./ex.json#user-profile";
    let result = read_contents(yaml, Some(&options));
    assert!(result.openapi.is_some());
    let profile = component(&result, "profile");
    assert!(profile.get_ref().is_some());
    assert_eq!(profile.get_schema().as_deref(), Some("https://json-schema.org/draft/2020-12/schema"));
    assert_eq!(profile.get_anchor().as_deref(), Some("foo"));
    assert_eq!(profile.get_id().as_deref(), Some("profile-id"));
    assert_eq!(profile.get_exclusive_maximum_value().unwrap().to_i32_wrapping(), 12);
    assert_eq!(profile.get_exclusive_minimum_value().unwrap().to_i32_wrapping(), 1);
    assert_eq!(profile.get_comment().as_deref(), Some("end user should not see this comment"));
    assert!(profile.get_types().unwrap().contains("string"));
    assert!(profile.get_types().unwrap().contains("integer"));
}

#[test]
fn test_siblings_reference_json_schema4() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions::new();
    let yaml = "openapi: 3.1.0\ninfo:\n  title: siblings JSONSchema\n  version: 1.0.0\nservers:\n  - url: /\npaths: { }\ncomponents:\n  schemas:\n    profile:\n      const: sales\n      contentEncoding: base64\n      contentMediaType: text/html\n      contentSchema:\n        type: string\n      $ref: ./ex.json#user-profile";
    let result = read_contents(yaml, Some(&options));
    assert!(result.openapi.is_some());
    let profile = component(&result, "profile");
    assert!(profile.get_ref().is_some());
    assert_eq!(profile.get_const(), Some(JavaObject::string("sales")));
    assert_eq!(profile.get_content_encoding().as_deref(), Some("base64"));
    assert_eq!(profile.get_content_media_type().as_deref(), Some("text/html"));
    assert!(profile.get_content_schema().is_some());
    assert!(profile.get_content_schema().unwrap().get_types().unwrap().contains("string"));
}

#[test]
fn test_siblings_reference_json_schema5() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions::new();
    let yaml = "openapi: 3.1.0\ninfo:\n  title: siblings JSONSchema\n  version: 1.0.0\nservers:\n  - url: /\npaths: { }\ncomponents:\n  schemas:\n    ContainsSchema:\n      type: array\n      contains:\n        type: integer\n      minContains: 2\n      maxContains: 4\n      uniqueItems: true\n    Profile:\n      propertyNames:\n        pattern: ^[A-Za-z_][A-Za-z0-9_]*$\n      $ref: ./ex.json#user-profile\n      unevaluatedProperties:\n         type: object\n      unevaluatedItems:\n          type: object\n    Person:\n      type: array\n      prefixItems:\n        - type: string\n          description: Name\n        - type: integer\n          description: Age\n      minItems: 2\n      maxItems: 2\n    Patient:\n      type: object\n      properties:\n        patientId: { }\n      patientDetails:\n        type: object\n    PatientPerson:\n      allOf:\n        - $ref: \"#/components/schemas/Person\"\n        - $ref: \"#/components/schemas/Patient\"\n      unevaluatedProperties: false\n      $ref: ./ex.json#patient-person";
    let result = read_contents(yaml, Some(&options));
    assert!(result.openapi.is_some());
    let profile = component(&result, "Profile");
    let contains_schema = component(&result, "ContainsSchema");
    let person_schema = component(&result, "Person");
    let patient_person_schema = component(&result, "PatientPerson");
    assert!(profile.get_ref().is_some());
    assert!(contains_schema.get_contains().is_some());
    assert_eq!(contains_schema.get_max_contains(), Some(4));
    assert_eq!(contains_schema.get_min_contains(), Some(2));
    assert!(person_schema.get_prefix_items().is_some());
    assert_eq!(person_schema.get_max_items(), Some(2));
    assert_eq!(person_schema.get_min_items(), Some(2));
    assert!(contains_schema.get_unique_items().is_some());
    assert_eq!(profile.get_property_names().unwrap().get_pattern().as_deref(), Some("^[A-Za-z_][A-Za-z0-9_]*$"));
    let up = profile.get_unevaluated_properties().expect("instanceof Schema");
    assert!(up.get_types().unwrap().contains("object"));
    let pup = patient_person_schema.get_unevaluated_properties().expect("instanceof Schema");
    assert_eq!(pup.get_boolean_schema_value(), Some(false));
    assert!(profile.get_unevaluated_items().is_some());
}

#[test]
fn test_siblings_reference_json_schema6() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions::new();
    let yaml = "openapi: 3.1.0\ninfo:\n  title: siblings JSONSchema\n  version: 1.0.0\nservers:\n  - url: /\npaths: { }\ncomponents:\n  schemas:\n    Payment:\n      type: object\n      properties:\n        name:\n          type: string\n        credit_card:\n          type: number\n        billing_address:\n          type: string\n      required:\n        - name\n      dependentRequired:\n        credit_card:\n          - billing_address\n    PaymentMethod:\n      type: object\n      properties:\n        name:\n          type: string\n        credit_card:\n          type: number\n      required:\n        - name\n      dependentSchemas:\n        credit_card:\n          properties:\n            billing_address:\n              type: string\n          required:\n            - billing_address\n    IfTest:\n      title: Person\n      type: object\n      properties:\n        country:\n          type: string\n          widget: Select\n          enum:\n            - usa\n            - canada\n            - eu\n          default: eu\n      required:\n        - country\n      if:\n        properties:\n          country:\n            type: string\n            const: canada\n      then:\n        properties:\n          maple_trees:\n            type: number\n      else:\n        properties:\n          accept:\n            type: boolean\n            const: true\n        required:\n          - accept\n";
    let result = read_contents(yaml, Some(&options));
    assert!(result.openapi.is_some());
    let it_test = component(&result, "IfTest");
    assert!(it_test.get_if().unwrap().get_properties().unwrap().contains_key("country"));
    assert!(it_test.get_then().unwrap().get_properties().unwrap().contains_key("maple_trees"));
    assert!(it_test.get_else().unwrap().get_properties().unwrap().contains_key("accept"));
    let payment = component(&result, "Payment");
    assert!(payment.get_dependent_required().unwrap().get("credit_card").is_some());
    let _payment_method = component(&result, "PaymentMethod");
}

#[test]
fn test_examples_json_schema() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions::new();
    let yaml = "openapi: 3.1.0\ninfo:\n  title: examples JSONSchema\n  version: 1.0.0\nservers:\n  - url: /\npaths: { }\ncomponents:\n  schemas:\n    Fruit:\n      type: string\n      example: kiwi\n      examples:\n        - apple\n        - orange\n    Error:\n      type: object\n      example: wrong\n      properties:\n        code:\n          type: integer\n        message:\n          type: string\n      examples:\n        - code: 123\n          message: Oops...\n        - code: 456\n          message: Feature is not available for your plan\n    ExampleSchema:\n      type: object\n      example: foo\n";
    let result = read_contents(yaml, Some(&options));
    assert!(result.openapi.is_some());
    let fruit = component(&result, "Fruit");
    assert_eq!(fruit.get_example(), Some(JavaObject::string("kiwi")));
    assert!(fruit.get_examples().is_some());
    let error = component(&result, "Error");
    assert_eq!(error.get_example(), Some(JavaObject::string("wrong")));
    assert!(error.get_examples().unwrap().get(0).is_some_and(|e| !e.is_null()));
    let example_schema = component(&result, "ExampleSchema");
    assert_eq!(example_schema.get_example(), Some(JavaObject::string("foo")));
}

#[test]
fn test_arbitrary_keywords_json_schema() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions::new();
    let yaml = "openapi: 3.1.0\ninfo:\n  title: arbitrary keywords JSONSchema\n  version: 1.0.0\nservers:\n  - url: /\npaths: { }\ncomponents:\n  schemas:\n    Fruit:\n      deprecated: true\n      type: string\n      example: kiwi\n      examples:\n        - apple\n        - orange\n      arbitraryKeyword: test\n      x-normalExtension: extensionTest\n";
    let result = read_contents(yaml, Some(&options));
    assert!(result.openapi.is_some());
    let fruit = component(&result, "Fruit");
    assert!(fruit.get_extensions().unwrap().get("arbitraryKeyword").is_some());
    assert!(fruit.get_extensions().unwrap().get("x-normalExtension").is_some());
    assert_eq!(fruit.get_deprecated(), Some(true));
}

#[test]
fn test_tuples_json_schema() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions::new();
    let yaml = "openapi: 3.1.0\ninfo:\n  title: tuple JSONSchema\n  version: 1.0.0\nservers:\n  - url: /\npaths: { }\ncomponents:\n  schemas:\n    Tuple:\n      type: array\n      prefixItems:\n        - type: string\n          description: Name\n        - type: integer\n          description: Age\n      minItems: 2\n      maxItems: 2\n";
    let result = read_contents(yaml, Some(&options));
    assert!(result.openapi.is_some());
    let schema = component(&result, "Tuple").get_prefix_items().unwrap().get(0).expect("instanceof Schema");
    assert!(schema.get_types().unwrap().contains("string"));
    assert_eq!(schema.get_description().as_deref(), Some("Name"));
}

#[test]
fn test_not_default_schema_type() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { infer_schema_type: false, ..Default::default() };
    let yaml = "openapi: 3.1.0\ninfo:\n  title: ping test\n  version: '1.0'\nservers:\n  - url: 'http://localhost:8000/'\npaths:\n  /ping:\n    get:\n      operationId: pingGet\n      responses:\n        '201':\n          description: OK\ncomponents:\n  schemas:\n    AnyValue: {}\n    AnyValueWithDesc:\n      description: Can be any value - string, number, boolean, array or object.\n    AnyValueNullable:\n      nullable: true\n      description: Can be any value, including `null`.\n    AnyValueModel:\n      description: test any value\n      type: object\n      properties:\n        any_value:\n          $ref: \"#/components/schemas/AnyValue\"\n        any_value_with_desc:\n          $ref: \"#/components/schemas/AnyValueWithDesc\"\n        any_value_nullable:\n          $ref: \"#/components/schemas/AnyValueNullable\"\n    AnyValueModelInline:\n      description: test any value inline\n      type: object\n      properties:\n        any_value: {}\n        any_value_with_desc:\n          description: inline any value\n        any_value_nullable:\n          nullable: true\n          description: inline any value nullable\n        map_any_value:\n          additionalProperties: {}\n        map_any_value_with_desc:\n          additionalProperties: \n            description: inline any value\n        map_any_value_nullable:\n          additionalProperties:\n            nullable: true\n            description: inline any value nullable\n        array_any_value:\n          items: {}\n        array_any_value_with_desc:\n          items: \n            description: inline any value\n        array_any_value_nullable:\n          items:\n            nullable: true\n            description: inline any value nullable";
    let result = read_contents(yaml, Some(&options));
    assert!(result.openapi.is_some());
    let props = component(&result, "AnyValueModelInline").get_properties().unwrap();
    for name in ["map_any_value", "map_any_value_with_desc", "map_any_value_nullable", "array_any_value", "array_any_value_with_desc", "array_any_value_nullable"] {
        let schema = props.get(name).unwrap_or_else(|| panic!("{name}"));
        assert!(schema.get_type().is_none(), "{name}");
    }
    let schema = props.get("array_any_value_nullable").unwrap();
    assert!(schema.get_items().unwrap().get_nullable().is_none());
    assert!(schema.get_items().unwrap().get_extensions().unwrap().get("nullable").is_some());
}

#[test]
fn test30_non_ref_siblings_behavior() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions::new();
    let yaml = "openapi: 3.0.0\ninfo:\n  title: siblings JSONSchema\n  version: 1.0.0\nservers:\n  - url: /\npaths: { }\ncomponents:\n  schemas:\n    profile:\n      description: siblings refs\n      required:\n        - login\n        - password\n      maxItems: 2\n      $ref: ./ex.json#user-profile\n      properties:\n        login:\n          type: string\n        password:\n          type: string";
    let result = read_contents(yaml, Some(&options));
    assert!(result.openapi.is_some());
    let profile = component(&result, "profile");
    assert!(profile.get_ref().is_some());
    assert!(profile.get_max_items().is_none());
    assert!(profile.get_description().is_none());
    assert!(profile.get_required().is_none());
    assert!(profile.get_properties().is_none());
}

#[test]
fn test31_issue1801() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions::new();
    let result = read_location("3.1.0/issue-1801.yaml", Some(&options));
    assert!(result.openapi.is_some());
    let all_of = component(&result, "AllofWithTheLastEmptySchema").get_all_of().unwrap();
    let first_all_of = all_of.get(0).unwrap();
    assert_eq!(first_all_of.get_types().unwrap().items()[0], "number");
    assert_eq!(first_all_of.get_type(), None);
    let second_all_of = all_of.get(1).unwrap();
    assert!(second_all_of.get_types().is_none());
    assert_eq!(first_all_of.get_type(), None);

    // System.setProperty("bind-type", "true") ... finally System.setProperty("bind-type", "false")
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            swagger_parser::models::schema::set_bind_type(false);
        }
    }
    let _reset = Reset;
    swagger_parser::models::schema::set_bind_type(true);
    let result = read_location("3.1.0/issue-1801.yaml", Some(&options));
    assert!(result.openapi.is_some());
    let all_of = component(&result, "AllofWithTheLastEmptySchema").get_all_of().unwrap();
    let first_all_of = all_of.get(0).unwrap();
    assert_eq!(first_all_of.get_types().unwrap().items()[0], "number");
    assert_eq!(first_all_of.get_type().as_deref(), Some("number"));
    let second_all_of = all_of.get(1).unwrap();
    assert!(second_all_of.get_types().is_none());
    assert_eq!(second_all_of.get_type(), None);
}

#[test]
fn test31_issue1821() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, resolve_fully: true, ..Default::default() };
    let result = read_location("3.1.0/issue-1821.yaml", Some(&options));
    assert!(result.openapi.is_some());
    let id = component(&result, "Rule").get_properties().unwrap().get("id").unwrap();
    assert_eq!(id.get_types().unwrap().items()[0], "string");
}

// ---- safe URL resolving: the remote hosts are simulated (no network) ----

mod safe {

    use std::rc::Rc;
    use std::sync::Arc;
    use swagger_parser::java::JavaException;
    use swagger_parser::parser::hooks::{self, Hooks};
    use swagger_parser_safe_url_resolver::java_rt::inet::InetAddress;
    use swagger_parser_safe_url_resolver::NetUtils;

    /// DNS for the hosts the documents reference: the public petstore hosts get a public address, localhost
    /// the loopback address.
    struct FakeDns;
    impl NetUtils for FakeDns {
        fn get_host_by_name(&self, hostname: &str) -> swagger_parser::java::JResult<InetAddress> {
            match hostname {
                "localhost" => InetAddress::get_by_name("127.0.0.1"),
                "petstore.swagger.io" | "petstore3.swagger.io" => InetAddress::get_by_name("104.21.0.1"),
                h => Err(JavaException::new("java.net.UnknownHostException", h)),
            }
        }
    }

    /// The petstore hosts answer a Java client with 403 (what the upstream assertions expect of them).
    pub fn install() -> hooks::HooksGuard {
        hooks::install(Hooks {
            url_to_string: Some(Rc::new(|url: &str, _| {
                Some(Err(JavaException::new("java.io.IOException", format!("Server returned HTTP response code: 403 for URL: {url}"))))
            })),
            net_utils: Some(Arc::new(FakeDns)),
            ..Default::default()
        })
    }
}

fn check_messages(result: &SwaggerParseResult, allowed: &[&str]) {
    if let Some(m) = &result.messages {
        for message in m {
            assert!(allowed.iter().any(|a| message.contains(a)), "{message}");
        }
    }
}

#[test]
fn test31_safe_url_resolving() {
    let _g = common::setup(common::V3_MODULE);
    let _h = safe::install();
    let options = ParseOptions { resolve_fully: true, safely_resolve_url: true, remote_ref_allow_list: Some(vec![]), remote_ref_block_list: Some(vec![]), ..Default::default() };
    let result = read_location("safeResolving/oas31SafeUrlResolvingWithPetstore.yaml", Some(&options));
    check_messages(&result, &["Server returned HTTP response code: 403"]);
}

#[test]
fn test31_safe_url_resolving_with_blocked_url() {
    let _g = common::setup(common::V3_MODULE);
    let _h = safe::install();
    let options = ParseOptions {
        resolve_fully: true,
        safely_resolve_url: true,
        remote_ref_allow_list: Some(vec![]),
        remote_ref_block_list: Some(vec!["petstore3.swagger.io".into()]),
        ..Default::default()
    };
    let result = read_location("safeResolving/oas31SafeUrlResolvingWithPetstore.yaml", Some(&options));
    check_messages(
        &result,
        &["Server returned HTTP response code: 403", "URL is part of the explicit denylist. URL [https://petstore3.swagger.io/api/v3/openapi.json]"],
    );
}

#[test]
fn test31_safe_url_resolving_with_turned_off_safe_resolving() {
    let _g = common::setup(common::V3_MODULE);
    let _h = safe::install();
    let options = ParseOptions {
        resolve_fully: true,
        safely_resolve_url: false,
        remote_ref_allow_list: Some(vec![]),
        remote_ref_block_list: Some(vec!["petstore3.swagger.io".into()]),
        ..Default::default()
    };
    let result = read_location("safeResolving/oas31SafeUrlResolvingWithPetstore.yaml", Some(&options));
    check_messages(&result, &["Server returned HTTP response code: 403"]);
}

#[test]
fn test31_safe_url_resolving_with_localhost_and_blocked_url() {
    let _g = common::setup(common::V3_MODULE);
    let _h = safe::install();
    let options = ParseOptions { resolve_fully: true, safely_resolve_url: true, ..Default::default() };
    let result = read_location("safeResolving/oas31SafeUrlResolvingWithLocalhost.yaml", Some(&options));
    check_messages(&result, &["Server returned HTTP response code: 403", "IP is restricted"]);
}

#[test]
fn test31_safe_url_resolving_with_localhost() {
    let _g = common::setup(common::V3_MODULE);
    let _h = safe::install();
    let options = ParseOptions {
        resolve_fully: true,
        safely_resolve_url: true,
        remote_ref_block_list: Some(vec!["petstore.swagger.io".into()]),
        ..Default::default()
    };
    let error = "URL is part of the explicit denylist. URL [https://petstore.swagger.io/v2/swagger.json]";
    let result = read_location("safeResolving/oas31SafeUrlResolvingWithLocalhost.yaml", Some(&options));
    check_messages(&result, &["Server returned HTTP response code: 403", "IP is restricted", error]);
}
