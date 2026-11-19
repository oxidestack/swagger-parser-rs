//! Port of `io.swagger.parser.OpenAPIParserTest` (the `swagger-parser` module).
//!
//! `testIssue1086` is not ported: it has no `@Test` annotation (and the class has none), so TestNG never
//! runs it. The commented-out `test30Url` is not ported either.

mod common;

use jackson::{BigDecimal, JavaObject};
use swagger_parser::OpenAPIParser;
use swagger_parser::core::{ParseOptions, SwaggerParseResult};
use swagger_parser::java::JMap;
use swagger_parser::models::{ApiResponse, OpenAPI, Operation, PathItem, Schema, SchemaKind, json_pretty, yaml_pretty};

/// `result.getMessages()` (non-null in every use below)
fn messages(r: &SwaggerParseResult) -> Vec<String> {
    r.messages.clone().expect("messages")
}

fn api(r: &SwaggerParseResult) -> OpenAPI {
    r.openapi.clone().expect("openAPI")
}

/// `openAPI.getComponents().getSchemas()`
fn schemas(api: &OpenAPI) -> JMap<Schema> {
    api.get_components().unwrap().get_schemas().unwrap()
}

/// `openAPI.getComponents().getSchemas().get(name)`
fn schema(api: &OpenAPI, name: &str) -> Schema {
    schemas(api).get(name).unwrap()
}

/// `schema.getProperties().get(name)`
fn prop(s: &Schema, name: &str) -> Option<Schema> {
    s.get_properties().unwrap().get(name)
}

/// `openAPI.getPaths().get(path)`
fn path(api: &OpenAPI, p: &str) -> PathItem {
    api.get_paths().unwrap().get(p).unwrap()
}

/// `operation.getResponses().get(code)`
fn response(op: &Operation, code: &str) -> ApiResponse {
    op.get_responses().unwrap().get(code).unwrap()
}

/// `response.getContent().get(mediaType).getSchema()`
fn content_schema(r: &ApiResponse, media_type: &str) -> Schema {
    r.get_content().unwrap().get(media_type).unwrap().get_schema().unwrap()
}

/// `operation.getRequestBody().getContent().get(mediaType).getSchema()`
fn request_body_schema(op: &Operation, media_type: &str) -> Schema {
    op.get_request_body().unwrap().get_content().unwrap().get(media_type).unwrap().get_schema().unwrap()
}

fn java_string(o: Option<JavaObject>) -> Option<String> {
    o.map(|o| o.to_java_string())
}

#[test]
fn test_npe_1685() {
    let _g = common::setup(common::PARSER_MODULE);
    let open_api_parser = OpenAPIParser::new();
    let options = ParseOptions { resolve_fully: true, ..Default::default() };
    let swagger_parse_result = open_api_parser.read_location("issue1685.json", None, Some(&options));
    assert!(swagger_parse_result.openapi.is_none());
    assert!(swagger_parse_result.messages.is_some());
    let messages = messages(&swagger_parse_result);
    assert!(messages.len() == 2);
    assert_eq!(messages[0], "attribute notswagger is unexpected");
    assert_eq!(messages[1], "attribute swagger is missing");
}

#[test]
fn test_issue1608() {
    let _g = common::setup(common::PARSER_MODULE);
    let options = ParseOptions { resolve_fully: true, ..Default::default() };
    let open_api_parser = OpenAPIParser::new();
    let swagger_parse_result = open_api_parser.read_location("issue1608.json", None, Some(&options));
    let schema = request_body_schema(&path(&api(&swagger_parse_result), "/pet").get_put().unwrap(), "application/json");
    assert_eq!(schema.get_required().unwrap().size(), 1);
}

#[test]
fn test_issue1143() {
    let _g = common::setup(common::PARSER_MODULE);
    let options = ParseOptions { resolve: true, ..Default::default() };
    let result = OpenAPIParser::new().read_location("issue-1143.json", None, Some(&options));
    assert!(result.openapi.is_some());
    assert!(schemas(&api(&result)).get("RedisResource").is_some());
    assert!(schemas(&api(&result)).get("identificacion_usuario_aplicacion").is_some());
}

#[test]
fn test_issue749() {
    let _g = common::setup(common::PARSER_MODULE);
    let options = ParseOptions { resolve: true, ..Default::default() };
    let result = OpenAPIParser::new().read_location("issue749-main.yaml", None, Some(&options));

    let open_api = result.openapi.clone();
    assert!(open_api.is_some());
    let open_api = open_api.unwrap();

    let components = open_api.get_components();
    assert!(components.is_some());
    let components = components.unwrap();

    let path_item = open_api.get_paths().unwrap().get("/some/ping");
    assert!(path_item.is_some());
    let parameters = path_item.unwrap().get_get().unwrap().get_parameters();
    assert!(parameters.is_some());
    let parameters = parameters.unwrap();
    assert_eq!(parameters.size(), 1);
    let p0 = parameters.get(0).unwrap();
    assert_eq!(p0.get_name().as_deref(), Some("i"));
    assert!(p0.get_schema().is_some());
    assert_eq!(p0.get_schema().unwrap().get_ref().as_deref(), Some("#/components/schemas/SomeId"));

    let schemas = components.get_schemas();
    assert!(schemas.is_some());
    let schemas = schemas.unwrap();
    assert_eq!(schemas.size(), 1);
    assert!(schemas.get("SomeId").is_some());
}

#[test]
fn test_simple() {
    let _g = common::setup(common::PARSER_MODULE);
    let result = OpenAPIParser::new().read_location("petstore.yaml", None, None);

    assert!(result.openapi.is_some());
    assert_eq!(api(&result).get_openapi().as_deref(), Some("3.0.1"));
}

#[test]
fn test_issue887() {
    let _g = common::setup(common::PARSER_MODULE);
    let _options = ParseOptions::default();
    let result = OpenAPIParser::new().read_location("apiWithMultipleTags.json", None, None);
    println!("{}", common::java_list(&messages(&result)));
    assert!(result.openapi.is_some());
    assert_eq!(messages(&result)[1], "attribute tags.sample is repeated");
}

#[test]
fn test_issue895() {
    let _g = common::setup(common::PARSER_MODULE);
    let result = OpenAPIParser::new().read_location("issue895.yaml", None, None);
    assert!(result.openapi.is_some());
    assert_eq!(messages(&result)[0], "info.contact.test");
    assert_eq!(messages(&result)[1], "info.license.test1");
}

#[test]
fn test_issue892() {
    let _g = common::setup(common::PARSER_MODULE);
    let result = OpenAPIParser::new().read_location("issue892-main.yaml", None, None);

    assert_eq!(messages(&result).len(), 1);
    assert!(result.openapi.is_some());
    assert_eq!(api(&result).get_openapi().as_deref(), Some("3.0.1"));
}

#[test]
fn test_issue934() {
    let _g = common::setup(common::PARSER_MODULE);
    let result = OpenAPIParser::new().read_location("issue-934.yaml", None, None);

    assert_eq!(messages(&result).len(), 1);
    assert!(result.openapi.is_some());
    assert_eq!(api(&result).get_openapi().as_deref(), Some("3.0.1"));
}

#[test]
fn test_issue768() {
    let _g = common::setup(common::PARSER_MODULE);
    let options = ParseOptions { resolve: true, ..Default::default() };
    let result = OpenAPIParser::new().read_location("issue768-main.yaml", None, Some(&options));

    let open_api = result.openapi.clone();
    assert!(open_api.is_some());

    let components = open_api.unwrap().get_components();
    assert!(components.is_some());

    let schemas = components.unwrap().get_schemas();
    assert!(schemas.is_some());

    assert_eq!(schemas.unwrap().size(), 1);
}

#[test]
fn test_converter_with_flatten() {
    let _g = common::setup(common::PARSER_MODULE);
    let yaml = "swagger: \"2.0\"\n\
                info:\n  description: \"Foo\"\n  version: \"1.0.0\"\n\
                host: \"something.com\"\n\
                basePath: \"/\"\n\
                schemes:\n  - \"https\"\n\
                consumes:\n  - \"application/json\"\n\
                produces:\n  - \"application/json\"\n\
                paths:\n  /example:\n    get:\n      responses:\n        200:\n          description: \"OK\"\n          schema:\n            $ref: \"#/definitions/Foo\"\n    parameters: []\n\
                definitions:\n  Foo:\n    type: \"object\"\n    required:\n    properties:\n      nested:\n        type: \"object\"\n        properties:\n          color:\n            type: \"string\"";

    let options = ParseOptions { resolve: true, flatten: true, ..Default::default() };
    let result = OpenAPIParser::new().read_contents(yaml, None, Some(&options));
    let open_api = api(&result);
    assert_eq!(schemas(&open_api).size(), 2);
}

#[test]
fn test30() {
    let _g = common::setup(common::PARSER_MODULE);
    let json = "{\n  \"openapi\": \"3.0.1\",\n  \"info\": {\n    \"title\": \"Swagger Petstore\",\n    \"description\": \"This is a sample server Petstore server. You can find out more about Swagger at [http://swagger.io](http://swagger.io) or on [irc.freenode.net, #swagger](http://swagger.io/irc/). For this sample, you can use the api key `special-key` to test the authorization filters.\",\n    \"termsOfService\": \"http://swagger.io/terms/\",\n    \"contact\": {\n      \"email\": \"apiteam@swagger.io\"\n    },\n    \"license\": {\n      \"name\": \"Apache 2.0\",\n      \"url\": \"http://www.apache.org/licenses/LICENSE-2.0.html\"\n    },\n    \"version\": \"1.0.0\"\n  }\n}";

    let options = ParseOptions { resolve: true, ..Default::default() };
    let result = OpenAPIParser::new().read_contents(json, None, Some(&options));

    assert!(result.openapi.is_some());
    assert_eq!(api(&result).get_openapi().as_deref(), Some("3.0.1"));
}

#[test]
fn test_parsing_prettified_extensions() {
    let _g = common::setup(common::PARSER_MODULE);
    let json = "{\n  \"openapi\": \"3.0.1\",\n  \"x-some-extension\": \"some-value\"\n}";

    let result = OpenAPIParser::new().read_contents(json, None, None);
    let open_api = result.openapi.clone();
    assert!(open_api.is_some());
    let open_api = open_api.unwrap();
    assert!(open_api.get_extensions().is_some());
    assert_eq!(java_string(open_api.get_extensions().unwrap().get("x-some-extension")).as_deref(), Some("some-value"));

    let pretty_json = json_pretty(&open_api).unwrap();

    let pretty_result = OpenAPIParser::new().read_contents(&pretty_json, None, None);
    let pretty_open_api = pretty_result.openapi.clone();
    assert!(pretty_open_api.is_some());
    let pretty_open_api = pretty_open_api.unwrap();
    assert!(pretty_open_api.get_extensions().is_some());
    assert_eq!(java_string(pretty_open_api.get_extensions().unwrap().get("x-some-extension")).as_deref(), Some("some-value"));
}

#[test]
fn test_issue799() {
    let _g = common::setup(common::PARSER_MODULE);
    let open_api_parser = OpenAPIParser::new();
    let options = ParseOptions { resolve: true, flatten: true, ..Default::default() };

    let open_api = api(&open_api_parser.read_location("issue799.json", None, Some(&options)));
    assert_eq!(prop(&schema(&open_api, "v1beta3.Binding"), "metadata").unwrap().get_ref().as_deref(), Some("#/components/schemas/v1beta3.ObjectMeta"));
    let body_parameter = path(&open_api, "/api/v1beta3/namespaces/{namespaces}/bindings").get_post().unwrap();
    assert_eq!(request_body_schema(&body_parameter, "*/*").get_ref().as_deref(), Some("#/components/schemas/v1beta3.Binding"));
    let op = path(&open_api, "/api/v1beta3/namespaces/{namespaces}/componentstatuses/{name}").get_get().unwrap();
    assert_eq!(content_schema(&response(&op, "200"), "application/json").get_ref().as_deref(), Some("#/components/schemas/v1beta3.ComponentStatus"));
    let op = path(&open_api, "/api/v1beta3/namespaces/{namespaces}/componentstatuses").get_get().unwrap();
    assert_eq!(content_schema(&response(&op, "200"), "application/json").get_ref().as_deref(), Some("#/components/schemas/v1beta3.ComponentStatusList"));
    let conditions_property = prop(&schema(&open_api, "v1beta3.ComponentStatus"), "conditions").unwrap();
    assert!(conditions_property.is_array_schema());
    let items = conditions_property.get_items().unwrap();
    assert!(items.get_ref().is_some());
    assert_eq!(items.get_ref().as_deref(), Some("#/components/schemas/v1beta3.ObjectReference"));
}

#[test]
fn test_issue813() {
    let _g = common::setup(common::PARSER_MODULE);
    let input_spec = r##"{
  "swagger": "2.0",
  "info": {
    "description": "This is a sample server Petstore server.  You can find out more about Swagger at <a href=\"http://swagger.io\">http://swagger.io</a> or on irc.freenode.net, #swagger.  For this sample, you can use the api key \"special-key\" to test the authorization filters",
    "version": "1.0.0",
    "title": "Swagger Petstore",
    "termsOfService": "http://helloreverb.com/terms/",
    "contact": {
      "email": "apiteam@wordnik.com"
    },
    "license": {
      "name": "Apache-2.0",
      "url": "http://www.apache.org/licenses/LICENSE-2.0.html"
    }
  },
  "host": "petstore.swagger.io",
  "basePath": "/v2",
  "schemes": [
    "http"
  ],
  "paths": {
    "/pet": {
      "post": {
        "tags": [
          "pet"
        ],
        "summary": "Add a new pet to the store",
        "description": "",
        "operationId": "addPet",
        "consumes": [
          "application/json",
          "application/xml"
        ],
        "produces": [
          "application/json",
          "application/xml"
        ],
        "parameters": [{
          "in": "body",
          "name": "body",
          "description": "Pet object that needs to be added to the store",
          "required": false,
          "schema": {
            "$ref": "#/definitions/Pet"
          }
        }],
        "responses": {
          "405": {
            "description": "Invalid input"
          }
        },
        "security": [{
          "petstore_auth": [
            "write:pets",
            "read:pets"
          ]
        }]
      },
      "put": {
        "tags": [
          "pet"
        ],
        "summary": "Update an existing pet",
        "description": "",
        "operationId": "updatePet",
        "consumes": [
          "application/json",
          "application/xml"
        ],
        "produces": [
          "application/json",
          "application/xml"
        ],
        "parameters": [{
          "in": "body",
          "name": "body",
          "description": "Pet object that needs to be added to the store",
          "required": false,
          "schema": {
            "$ref": "#/definitions/Pet"
          }
        }],
        "responses": {
          "405": {
            "description": "Validation exception"
          },
          "404": {
            "description": "Pet not found"
          },
          "400": {
            "description": "Invalid ID supplied"
          }
        },
        "security": [{
          "petstore_auth": [
            "write:pets",
            "read:pets"
          ]
        }]
      }
    },
    "securityDefinitions": {
      "api_key": {
        "type": "apiKey",
        "name": "api_key",
        "in": "header"
      },
      "petstore_auth": {
        "type": "oauth2",
        "authorizationUrl": "http://petstore.swagger.io/api/oauth/dialog",
        "flow": "implicit",
        "scopes": {
          "write:pets": "modify pets in your account",
          "read:pets": "read your pets"
        }
      }
    },
    "definitions": {
      "Pet": {
        "required": [
          "name",
          "photoUrls"
        ],
        "properties": {
          "id": {
            "type": "integer",
            "format": "int64"
          },
          "category": {
            "$ref": "#/definitions/Category"
          },
          "name": {
            "type": "string",
            "example": "doggie"
          },
          "photoUrls": {
            "type": "array",
            "xml": {
              "name": "photoUrl",
              "wrapped": true
            },
            "items": {
              "type": "string"
            }
          },
          "tags": {
            "type": "array",
            "xml": {
              "name": "tag",
              "wrapped": true
            },
            "items": {
              "$ref": "#/definitions/Tag"
            }
          },
          "status": {
            "type": "string",
            "description": "pet status in the store",
            "enum": [
              "available",
              "pending",
              "sold"
            ]
          }
        },
        "xml": {
          "name": "Pet"
        }
      }
    }
  }
}"##;

    let options = ParseOptions { resolve: true, flatten: true, ..Default::default() };
    let result = OpenAPIParser::new().read_contents(input_spec, None, Some(&options));
    assert!(result.openapi.is_some());
}

#[test]
fn test_issue258() {
    let _g = common::setup(common::PARSER_MODULE);
    let options = ParseOptions { resolve: true, ..Default::default() };
    let result = OpenAPIParser::new().read_location("duplicateOperationId.json", None, Some(&options));

    println!("{}", common::java_list(&messages(&result)));
    assert!(result.openapi.is_some());
    assert_eq!(messages(&result)[0], "attribute paths.'/pets/{id}'(post).operationId is repeated");
}

#[test]
fn test_issue_relative_refs2() {
    let _g = common::setup(common::PARSER_MODULE);
    let location = "exampleSpecs/specs/my-domain/test-api/v1/test-api-swagger_v1.json";
    let po = ParseOptions { resolve: true, ..Default::default() };
    let result = OpenAPIParser::new().read_location(location, None, Some(&po));

    assert!(result.openapi.is_some());
    let open_api = api(&result);

    let resources = prop(&schema(&open_api, "confirmMessageType_v01"), "resources").unwrap();
    assert!(resources.is_array_schema());

    let prop = prop(&resources.get_items().unwrap(), "resourceID").unwrap();

    assert_eq!(prop.get_ref().as_deref(), Some("#/components/schemas/simpleIDType_v01"));
}

#[test]
fn test_issue_relative_refs1() {
    let _g = common::setup(common::PARSER_MODULE);
    let location = "specs2/my-domain/test-api/v1/test-api-swagger_v1.json";
    let po = ParseOptions { resolve: true, ..Default::default() };
    let result = OpenAPIParser::new().read_location(location, None, Some(&po));

    assert!(result.openapi.is_some());
    let open_api = api(&result);

    let testing_api = prop(&schema(&open_api, "test-api-schema_v01"), "testingApi").unwrap();
    assert!(testing_api.is_array_schema());

    let prop = prop(&testing_api.get_items().unwrap(), "itemID").unwrap();

    assert_eq!(prop.get_ref().as_deref(), Some("#/components/schemas/simpleIDType_v01"));
}

#[test]
fn test_issue879() {
    let _g = common::setup(common::PARSER_MODULE);
    let open_api_parser = OpenAPIParser::new();
    let options = ParseOptions::default();
    let open_api = api(&open_api_parser.read_location("issue_879.yaml", None, Some(&options)));

    let r#ref = path(&open_api, "/register").get_post().unwrap().get_callbacks().unwrap().get("myEvent").unwrap().get_ref();
    assert_eq!(r#ref.as_deref(), Some("#/components/callbacks/callbackEvent"));
}

#[test]
fn test_issue959() {
    let _g = common::setup(common::PARSER_MODULE);
    let open_api_parser = OpenAPIParser::new();
    let result = open_api_parser.read_location("issue959.json", None, None);
    assert_eq!(messages(&result)[0], "paths.'/pets/{petId}'(get).parameters. There are duplicate parameter values");

    let result = open_api_parser.read_location("issue959PathLevelDuplication.json", None, None);
    assert_eq!(messages(&result)[0], "paths.'/pets'. There are duplicate parameter values");
}

#[test]
fn test_issue1003_extensions_classloader() {
    let _g = common::setup(common::PARSER_MODULE);
    // JDK-specific: Java swaps the thread context class loader for an empty URLClassLoader around the call
    // (the parser extensions must still be found through the parser's own class loader). There is no class
    // loader in the port: the extension list is fixed, so the call runs as is.
    let api = OpenAPIParser::new().try_read_location("src/test/resources/petstore.yaml", None, None).unwrap();
    assert!(api.is_some());
}

#[test]
fn test_issue1070() {
    let _g = common::setup(common::PARSER_MODULE);
    let result = OpenAPIParser::new().read_location("issue1070.yaml", None, None);
    let required = schema(&api(&result), "AmountAndCurrency").get_required().unwrap();
    assert_eq!(required.size(), 2);
    assert!(required.contains(&"Amount".to_string()));
    assert!(required.contains(&"Currency".to_string()));
}

#[test]
fn test_issue1433_resolve_schema_without_type() {
    let _g = common::setup(common::PARSER_MODULE);
    let open_api_parser = OpenAPIParser::new();
    let options = ParseOptions { resolve_fully: true, ..Default::default() };

    let open_api = api(&open_api_parser.read_location("issue_1433-resolve-schema-without-type.yaml", None, Some(&options)));
    let request_body_schema = request_body_schema(&path(&open_api, "/foo").get_post().unwrap(), "application/json");

    let properties = request_body_schema.get_properties().unwrap();
    assert_eq!(properties.size(), 2);

    let bar = properties.get("bar").unwrap();
    assert_eq!(bar.kind(), SchemaKind::String);

    let input = properties.get("input").unwrap();
    assert_eq!(input.kind(), SchemaKind::Schema);

    let input_properties = input.get_properties();
    assert!(input_properties.is_some());
    let input_properties = input_properties.unwrap();
    assert_eq!(input_properties.size(), 1);

    let baz = input_properties.get("baz").unwrap();
    assert_eq!(baz.kind(), SchemaKind::String);
}

#[test]
fn test_multiple_of_between_zero_and_one() {
    let _g = common::setup(common::PARSER_MODULE);
    let spec = "openapi: 3.0.0\n\
                info:\n  version: 0.0.0\n  title: \"test\"\n\
                components:\n  schemas:\n    Test:\n      type: object\n      properties:\n        decimal_value:\n          type: number\n          multipleOf: 0.3\n";

    let open_api_parser = OpenAPIParser::new();
    let options = ParseOptions::default();

    let open_api = api(&open_api_parser.read_contents(spec, None, Some(&options)));
    let schema = schema(&open_api, "Test");
    assert!(schema.is_object_schema());
    let decimal_value = prop(&schema, "decimal_value").unwrap();
    let multiple_of = decimal_value.get_multiple_of().unwrap();
    // `new BigDecimal("0.3", new MathContext(multipleOf.precision()))`: a precision of at least one digit
    // leaves 0.3 unrounded.
    assert!(multiple_of.precision() >= 1);
    assert_eq!(multiple_of, BigDecimal::parse("0.3").unwrap());
}

#[test]
fn test_convert_windows_paths_to_unix_when_resolving_server_paths() {
    let _g = common::setup(common::PARSER_MODULE);
    let options = ParseOptions { resolve: true, ..Default::default() };
    let result = OpenAPIParser::new().read_location("exampleSpecs\\specs\\issue1553.yaml", None, Some(&options));

    assert_eq!(Some("/api/customer1/v1"), api(&result).get_servers().unwrap().get(0).unwrap().get_url().as_deref());
}

/// convert response schema
#[test]
fn test_issue1552() {
    let _g = common::setup(common::PARSER_MODULE);
    let options = ParseOptions { resolve: true, ..Default::default() };
    let parser = OpenAPIParser::new();
    let result = parser.read_contents(
        "swagger: \"2.0\"\n\
         info:\n  version: \"1.0\"\n  title: \"test\"\n\
         host: \"foo\"\n\
         paths:\n  /example:\n    get:\n      consumes:\n        - \"application/json\"\n      produces:\n        - \"application/json\"\n      responses:\n        200:\n          $ref: \"#/responses/something\"\n\
         responses:\n  something:\n    description: \"my response\"\n    schema:\n      title: \"my schema\"\n      properties:\n        foo:\n          type: string\n",
        None,
        Some(&options),
    );
    let op = path(&api(&result), "/example").get_get().unwrap();
    let schema = content_schema(&response(&op, "200"), "application/json");
    assert!(schema.get_title().is_some()); // found title
    assert!(prop(&schema, "foo").is_some());
}

/// Issue 2269: preserve x-nullable in shared responses for swagger 2.0 specs
#[test]
fn test_swagger2_shared_response_nullable() {
    let _g = common::setup(common::PARSER_MODULE);
    let options = ParseOptions { resolve: true, resolve_fully: true, ..Default::default() };

    let result = OpenAPIParser::new().read_location("issue2269.yaml", None, Some(&options));

    assert!(result.openapi.is_some());
    let open_api = api(&result);

    let first_schema = |p: &str| {
        let op = path(&open_api, p).get_get().unwrap();
        response(&op, "200").get_content().unwrap().values().into_iter().next().unwrap().get_schema().unwrap()
    };

    let endpoint1_field = prop(&first_schema("/endpoint1"), "optional_field");
    assert!(endpoint1_field.is_some(), "Endpoint 1 should have optional_field");
    assert_eq!(endpoint1_field.unwrap().get_nullable(), Some(true), "Endpoint 1 optional_field should be nullable");

    let endpoint2_field = prop(&first_schema("/endpoint2"), "optional_field");
    assert!(endpoint2_field.is_some(), "Endpoint 2 should have optional_field");
    assert_eq!(endpoint2_field.unwrap().get_nullable(), Some(true), "Endpoint 2 optional_field should be nullable");
}

/// convert response schema
#[test]
fn test_issue1552_additional_props() {
    let _g = common::setup(common::PARSER_MODULE);
    let options = ParseOptions { resolve: true, infer_schema_type: false, ..Default::default() };
    let parser = OpenAPIParser::new();
    let result = parser.read_contents(
        "swagger: '2.0'\n\
         info:\n  title: Some API\n  description: >-\n    This is the Some Api\n  version: 2.0.0\n\
         basePath: /somepath\n\
         schemes:\n  - https\n\
         consumes:\n  - application/json\n\
         produces:\n  - application/json\n\
         paths:\n  /somepath:\n    get:\n      description: >\n        my description\n      operationId: MyGet\n      responses:\n        '200':\n          $ref: '#/responses/Response'\n\
         responses:\n  Response:\n    description: Response\n    schema:\n      type: object\n      required:\n        - Report\n      properties:\n        Report:\n          type: string\n      additionalProperties: false",
        None,
        Some(&options),
    );
    assert_eq!(
        yaml_pretty(&result).unwrap(),
        "messages: []\n\
         openAPI:\n  openapi: 3.0.1\n  info:\n    title: Some API\n    description: This is the Some Api\n    version: 2.0.0\n  servers:\n  - url: /somepath\n  paths:\n    /somepath:\n      get:\n        description: |\n          my description\n        operationId: MyGet\n        responses:\n          \"200\":\n            description: Response\n            content:\n              application/json:\n                schema:\n                  required:\n                  - Report\n                  type: object\n                  properties:\n                    Report:\n                      type: string\n                  additionalProperties: false\n  components:\n    responses:\n      Response:\n        description: Response\n        content:\n          application/json:\n            schema:\n              required:\n              - Report\n              type: object\n              properties:\n                Report:\n                  type: string\n              additionalProperties: false\n  x-original-swagger-version: \"2.0\"\n\
         openapi31: false\n"
    );
}
