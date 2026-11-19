//! Port of `io.swagger.v3.parser.test.OAIDeserializationTest`.

mod common;

use swagger_parser::core::ParseOptions;
use swagger_parser::java::JMap;
use swagger_parser::models::{
    yaml_string, ApiResponse, ApiResponses, Components, Content, MediaType, OpenAPI, Operation, PathItem, Paths, RequestBody, Schema, SpecVersion,
};
use swagger_parser::parser::OpenAPIV3Parser;

#[test]
fn test_deserialize_simple_definition() {
    let _g = common::setup(common::V3_MODULE);
    let json = "{\n  \"openapi\": \"3.0.1\",\n  \"info\": {\n    \"title\": \"Swagger Petstore\",\n    \"description\": \"This is a sample server Petstore server. You can find out more about Swagger at [http://swagger.io](http://swagger.io) or on [irc.freenode.net, #swagger](http://swagger.io/irc/). For this sample, you can use the api key `special-key` to test the authorization filters.\",\n    \"termsOfService\": \"http://swagger.io/terms/\",\n    \"contact\": {\n      \"email\": \"apiteam@swagger.io\"\n    },\n    \"license\": {\n      \"name\": \"Apache 2.0\",\n      \"url\": \"http://www.apache.org/licenses/LICENSE-2.0.html\"\n    },\n    \"version\": \"1.0.0\"\n  }\n}";
    let options = ParseOptions { resolve: true, ..Default::default() };
    let result = OpenAPIV3Parser::new().read_contents(json, None, Some(&options));
    assert!(result.openapi.is_some());
}

#[test]
fn test_issue911() {
    let _g = common::setup(common::V3_MODULE);
    let result = OpenAPIV3Parser::new().read_location("issue_911.yaml", None, None);
    assert_eq!(result.messages.as_ref().unwrap().len(), 1);
    assert!(result.openapi.is_some());
}

#[test]
fn test_deserialize_yaml_definition_missing_schema_issue1951() {
    let _g = common::setup(common::V3_MODULE);
    let api = OpenAPI::new();
    api.set_spec_version(SpecVersion::V30);
    api.set_components(Components::new());
    let error_response = ApiResponse::new().content(Content::new());
    error_response
        .description("My error type")
        .get_content()
        .unwrap()
        .add_media_type("application/json", MediaType::new().schema(Schema::composed_schema().ref_("NotAddedYet").type_("schema")));
    api.get_components().unwrap().set_responses(JMap::new_hash());
    api.get_components().unwrap().get_responses().unwrap().put("ErrorObj", error_response);
    let schema = Schema::composed_schema();
    schema.set_ref("ThingRequest");
    schema.set_type("schema");
    let content = Content::new().add_media_type("application/json", MediaType::new().schema(schema));
    let resp = ApiResponses::new();
    resp.add_api_response("401", ApiResponse::new().ref_("#/components/responses/ErrorObj"));
    resp.set_default(
        ApiResponse::new()
            .description("Default response is just a string")
            .content(Content::new().add_media_type("application/json", MediaType::new().schema(Schema::string_schema()))),
    );
    api.set_paths(Paths::new().add_path_item("/thingy", PathItem::new().post(Operation::new().responses(resp).request_body(RequestBody::new().content(content)))));

    let yaml = yaml_string(&api).unwrap();

    let options = ParseOptions { resolve: true, ..Default::default() };
    let result = OpenAPIV3Parser::new().read_contents(&yaml, None, Some(&options));
    assert!(result.openapi.is_some());
    let m = result.messages.clone().unwrap();
    assert!(m.contains(&"attribute info is missing".to_string()));
    assert!(m.contains(&"attribute components.responses.ErrorObj.content.'application/json'.schema.NotAddedYet is missing".to_string()));
    assert!(m.contains(&"attribute paths.'/thingy'(post).requestBody.content.'application/json'.schema.#/components/schemas/ThingRequest is missing".to_string()));
}

#[test]
fn test_deserialize_schema_with_default_property() {
    let _g = common::setup(common::V3_MODULE);
    let result = OpenAPIV3Parser::new().read_location("/schemas-default-value/default.yaml", None, None);
    assert_eq!(result.messages.as_ref().unwrap().len(), 0);
    assert!(result.openapi.is_some());
}

#[test]
fn test_deserialize_schema_with_null_default_property() {
    let _g = common::setup(common::V3_MODULE);
    let result = OpenAPIV3Parser::new().read_location("/schemas-default-value/defaultNull.yaml", None, None);
    assert_eq!(result.messages.as_ref().unwrap().len(), 0);
    assert!(result.openapi.is_some());
}

#[test]
fn test_deserialize_schema_with_null_default_and_nullable_true_property() {
    let _g = common::setup(common::V3_MODULE);
    let result = OpenAPIV3Parser::new().read_location("/schemas-default-value/defaultNullAndNullableTrue.yaml", None, None);
    assert_eq!(result.messages.as_ref().unwrap().len(), 0);
    assert!(result.openapi.is_some());
}

#[test]
fn test_deserialize_schema_with_null_default_and_nullable_false_property() {
    let _g = common::setup(common::V3_MODULE);
    let result = OpenAPIV3Parser::new().read_location("/schemas-default-value/defaultNullAndNullableFalse.yaml", None, None);
    let m = result.messages.clone().unwrap();
    assert_eq!(m.len(), 1);
    assert_eq!(m[0], "attribute components.schemas.Test.default is not of type `non-null string`");
    assert!(result.openapi.is_some());
}
