//! Port of `io.swagger.parser.test.V2ConverterTest` (swagger-parser-v2-converter).

mod common;

use jackson::{BigDecimal, JavaObject};
use swagger_parser::converter::SwaggerConverter;
use swagger_parser::core::{AuthorizationValue, ParseOptions, SwaggerParseResult, SwaggerParserExtension};
use swagger_parser::java::classpath::find_resource;
use swagger_parser::models::{json_string, AdditionalProperties, ApiResponse, OpenAPI, Operation, Parameter, ParameterStyle, PathItem, Schema, SchemaKind};

const PET_STORE_JSON: &str = "petstore.json";
const PET_STORE_YAML: &str = "petstore.yaml";
const PARAMETER_CONVERSION_JSON: &str = "parameter-conversion.json";
const ISSUE_2_JSON: &str = "issue-2.json";
const ISSUE_3_JSON: &str = "issue-3.json";
const ISSUE_4_JSON: &str = "issue-4.json";
const ISSUE_6_JSON: &str = "issue-6.json";
const ISSUE_8_JSON: &str = "issue-8.json";
const ISSUE_11_JSON: &str = "issue-11.json";
const ISSUE_13_JSON: &str = "issue-13.json";
const ISSUE_14_JSON: &str = "issue-14.json";
const ISSUE_15_JSON: &str = "issue-15.json";
const ISSUE_16_JSON: &str = "issue-16.json";
const ISSUE_17_JSON: &str = "issue-17.json";
const ISSUE_18_JSON: &str = "issue-18.json";
const ISSUE_19_JSON: &str = "issue-19.json";
const ISSUE_20_JSON: &str = "issue-20.json";
const ISSUE_21_JSON: &str = "issue-21.json";
const ISSUE_22_JSON: &str = "issue-22.json";
const ISSUE_23_JSON: &str = "issue-23.json";
const ISSUE_25_JSON: &str = "issue-25.json";
const ISSUE_26_JSON: &str = "issue-26.json";
const ISSUE_27_JSON: &str = "issue-27.json";
const ISSUE_28_JSON: &str = "issue-28.json";
const ISSUE_30_JSON: &str = "issue-30.json";
const ISSUE_31_JSON: &str = "issue-31.json";
const ISSUE_32_JSON: &str = "issue-32.json";
const ISSUE_33_JSON: &str = "issue-33.json";
const ISSUE_35_JSON: &str = "issue-35.json";
const ISSUE_36_JSON: &str = "issue-36.json";
const ISSUE_597_JSON: &str = "issue-597.json";
const ISSUE_599_JSON: &str = "issue-599.json";
const ISSUE_600_JSON: &str = "issue-600.json";
const ISSUE_455_JSON: &str = "issue-455.json";
const ISSUE_540_JSON: &str = "issue-540.json";
const ISSUE_647_JSON: &str = "issue-647.yaml";
const ISSUE_662_JSON: &str = "issue-662.yaml";
const ISSUE_672_JSON: &str = "issue-672.json";
const ISSUE_673_YAML: &str = "issue-673.yaml";
const ISSUE_676_JSON: &str = "issue-676.json";
const ISSUE_708_YAML: &str = "issue-708.yaml";
const ISSUE_745_YAML: &str = "issue-745.yaml";
const ISSUE_755_YAML: &str = "issue-755.yaml";
const ISSUE_740_YAML: &str = "issue-740.yaml";
const ISSUE_756_JSON: &str = "issue-756.json";
const ISSUE_758_JSON: &str = "issue-758.json";
const ISSUE_762_JSON: &str = "issue-762.json";
const ISSUE_765_YAML: &str = "issue-765.yaml";
const ISSUE_768_JSON: &str = "issue-786.json";
const ISSUE_820_YAML: &str = "issue-820.yaml";
const ISSUE_1032_YAML: &str = "issue-1032.yaml";
const ISSUE_1113_YAML: &str = "issue-1113.yaml";
const ISSUE_1164_YAML: &str = "issue-1164.yaml";
const ISSUE_1261_YAML: &str = "issue-1261.yaml";

const ISSUE_1715_YAML: &str = "issue-1715.yaml";

const ISSUE_1767_YAML: &str = "issue-1767.yaml";
const ISSUE_1796_YAML: &str = "issue-1796.yaml";
const ISSUE_2269_YAML: &str = "issue-2269.yaml";

const API_BATCH_PATH: &str = "/api/batch/";
const PETS_PATH: &str = "/pets";
const PET_FIND_BY_STATUS_PATH: &str = "/pet/findByStatus";
const PET_PATH: &str = "/pet";
const FILE_PATH: &str = "/file";
const POST_PATH: &str = "/post";
const LOGIN_PATH: &str = "/login";
const USERS_PATH: &str = "/users";
const APPLICATION_YAML: &str = "application/yaml";
const APPLICATION_JSON: &str = "application/json";
const PASSWORD_VALUE: &str = "p@55w0rd";
const PETSTORE_URL: &str = "http://petstore.swagger.io/api";
const VALUE: &str = "value";
const APPLICATION_PDF: &str = "application/pdf";
const BINARY_FORMAT: &str = "binary";
const ARRAY_TYPE: &str = "array";
const PET_SCHEMA: &str = "Pet";
const PET_TAG: &str = "pet";
const RESPONSE_200OK_COMPONENT: &str = "200OK";
const DEFAULT_RESPONSE_COMPONENT: &str = "DefaultResponse";
const SECURITY_SCHEMA_OAUTH2: &str = "OAuth2Implicit";
const AUTHORIZATION_URL: &str = "http://swagger.io/api/oauth/dialog";
const SCOPE_WRITE_PETS: &str = "write:pets";
const SCOPE_READ_PETS: &str = "read:pets";
const WRITE_PETS_VALUE: &str = "modify pets in your account";
const READ_PETS_VALUE: &str = "read your pets";
const EXTENDED_ERROR_MODEL: &str = "ExtendedErrorModel";
const ERROR_MODEL_REF: &str = "#/components/schemas/ErrorModel";
const MAP_OBJECTS_MODEL: &str = "MapOfObjects";
const OBJECT: &str = "object";
const OBJECT_REF: &str = "#/components/schemas/Object";
const USER_LOGIN_PATH: &str = "/user/login";
const X_RATE_LIMIT: &str = "X-Rate-Limit";
const X_EXPIRES_AFTER: &str = "X-Expires-After";
const X_RATE_LIMIT_DESCRIPTION: &str = "calls per hour allowed by the user";
const X_EXPIRES_AFTER_DESCRIPTION: &str = "date in UTC when token expires";
const X_EXAMPLE: &str = "x-example";
const CONTENT_TYPE: &str = "application/x-www-form-urlencoded";
const URLENCODED_CONTENT: &str = "application/x-www-form-urlencoded";
const PATTERN: &str = "^[a-zA-Z0-9]+$";
const FOO_PATH: &str = "/{foo}";
const FOO_VALUE: &str = "foooooo";
const CODE_EXAMPLE: &str = "2866bbb7-ba38-4da3-b6b6-25d1ec6c161f";
const EMAIL: &str = "email";
const PASSWORD: &str = "password";
const EMAIL_VALUE: &str = "bob@example.com";
const MONDAY_TUESDAY_VALUE: &str = "monday_tuesday";
const TUESDAY_WEDNESDAY_VALUE: &str = "tuesday_wednesday";
const WEDNESDAY_THURSDAY_VALUE: &str = "wednesday_thursday";
const ARTHUR_DENT_NAME: &str = "Arthur Dent";
const NAME: &str = "name";
const USER_MODEL: &str = "User";
const ID: &str = "id";
const FRIEND_IDS: &str = "friend_ids";
const ARRAY_OF_USERS_MODEL: &str = "ArrayOfUsers";
const HEAD_OPERATION: &str = "Head Operation";
const OPTIONS_OPERATION: &str = "Options Operation";
const AVAILABLE: &str = "available";
const PENDING: &str = "pending";
const SOLD: &str = "sold";
const ARRAY_VALUES: &str = "[{\"id\":-1,\"name\":\"Marvin the Paranoid Android\"},{\"id\":1000000,\"name\":\"Zaphod Beeblebrox\",\"friends\":[15]}]";
const SCHEMAS_A_REF: &str = "#/components/schemas/A";
const UNIX_TIMESTAMP_QUERY_PARAM: &str = "unixTimestampQuery";
const INTEGER_TYPE: &str = "integer";
const INT64_FORMAT: &str = "int64";

const MAX_LENGTH: i32 = 60;
const REQUIRED_SIZE: usize = 2;
const MIN_ITEMS: i32 = 1;
const PARAMETERS_SIZE: usize = 1;
const PROPERTIES_SIZE: usize = 4;
const ENUM_SIZE: usize = 3;
const MAXIMUM: i32 = 100;
const MIN_LENGTH: i32 = 3;
const NUMBER_VALUE_TWENTY: i32 = 20;
const MULTIPLE_OF_VALUE: f64 = 0.01;
const DEFAULT_VALUE: i32 = 11;
const EXAMPLE_8_NUMBER: i32 = 8;
const EXAMPLE_42_NUMBER: i32 = 42;

// ---------------------------------------------------------------------------------------------------------
// helpers

/// `new String(Files.readAllBytes(Paths.get(getClass().getClassLoader().getResource(file).toURI())))`
fn read_resource(file: &str) -> String {
    let path = find_resource(file).unwrap_or_else(|| panic!("resource {file}"));
    common::read_file(path.as_str())
}

/// `converter.readContents(s, null, options)`: an exception escaping it fails the test.
fn read_contents(s: &str, options: Option<&ParseOptions>) -> Option<SwaggerParseResult> {
    SwaggerConverter::new().try_read_contents(s, None, options).unwrap_or_else(|e| panic!("readContents threw {e:?}"))
}

/// `converter.readLocation(url, null, options)`: an exception escaping it fails the test.
fn read_location(url: &str, options: Option<&ParseOptions>) -> Option<SwaggerParseResult> {
    SwaggerConverter::new().try_read_location(url, None, options).unwrap_or_else(|e| panic!("readLocation threw {e:?}"))
}

/// `getConvertedOpenAPIFromJsonFile(file)`; the tests dereference the result, which fails when it is null.
fn get_converted_open_api_from_json_file(file: &str) -> OpenAPI {
    let swagger_as_string = read_resource(file);
    let parse_options = ParseOptions { resolve: false, ..Default::default() };
    let result = read_contents(&swagger_as_string, Some(&parse_options));
    assert!(result.is_some());
    result.unwrap().openapi.expect("getOpenAPI()")
}

fn path(oas: &OpenAPI, p: &str) -> PathItem {
    oas.get_paths().unwrap().get(p).unwrap()
}

fn get_op(oas: &OpenAPI, p: &str) -> Operation {
    path(oas, p).get_get().unwrap()
}

fn post_op(oas: &OpenAPI, p: &str) -> Operation {
    path(oas, p).get_post().unwrap()
}

fn params(op: &Operation) -> Vec<Parameter> {
    op.get_parameters().unwrap().to_vec()
}

fn response(op: &Operation, code: &str) -> ApiResponse {
    op.get_responses().unwrap().get(code).unwrap()
}

fn component_schema(oas: &OpenAPI, name: &str) -> Option<Schema> {
    oas.get_components().unwrap().get_schemas().unwrap().get(name)
}

fn prop(s: &Schema, name: &str) -> Schema {
    s.get_properties().unwrap().get(name).unwrap()
}

fn ext(extensions: Option<swagger_parser::java::JMap<JavaObject>>, name: &str) -> Option<JavaObject> {
    extensions.unwrap().get(name)
}

fn string(s: &str) -> Option<JavaObject> {
    Some(JavaObject::string(s))
}

fn integer(i: i32) -> Option<JavaObject> {
    Some(JavaObject::Integer(i))
}

/// `(ComposedSchema) schema` / `(ArraySchema) schema`: the cast fails for any other class
fn cast(s: Schema, kind: SchemaKind) -> Schema {
    assert_eq!(s.kind(), kind, "ClassCastException");
    s
}

// ---------------------------------------------------------------------------------------------------------
// tests

#[test]
fn test_convert_petstore() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let swagger_as_string = read_resource(PET_STORE_YAML);
    let result = read_contents(&swagger_as_string, None).unwrap();

    assert!(result.openapi.is_some());
}

#[test]
fn test_issue455() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_455_JSON);

    assert_eq!(PARAMETERS_SIZE, oas.get_paths().unwrap().size());

    let path_item = oas.get_paths().unwrap().get(API_BATCH_PATH);
    assert!(path_item.is_some());

    assert_eq!(PARAMETERS_SIZE, path_item.unwrap().get_get().unwrap().get_parameters().unwrap().size());
}

/// Missing array item type in parameters
#[test]
fn test_issue1() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(PET_STORE_JSON);
    let status_parameter = params(&get_op(&oas, PET_FIND_BY_STATUS_PATH)).into_iter().next();
    assert!(status_parameter.is_some());
    let schema = status_parameter.unwrap().get_schema().unwrap();
    assert!(schema.is_array_schema());
    let array_schema = schema;
    assert_eq!(Some(ARRAY_TYPE.to_string()), array_schema.get_type());
    assert_eq!(ENUM_SIZE, array_schema.get_items().unwrap().get_enum().unwrap().size());
}

/// Response Code
#[test]
fn test_issue2() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_2_JSON);
    assert_eq!(REQUIRED_SIZE, get_op(&oas, API_BATCH_PATH).get_responses().unwrap().size());
}

/// Servers when Hosts, basePath, scheme
#[test]
fn test_issue3() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_3_JSON);
    assert!(oas.get_servers().is_some());
    assert_eq!(Some(PETSTORE_URL.to_string()), oas.get_servers().unwrap().get(0).unwrap().get_url());
}

/// Contents in Responses
#[test]
fn test_issue4() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_4_JSON);
    assert!(response(&get_op(&oas, PETS_PATH), "200").get_content().is_some());
}

/// Tags are missing in the converted spec
#[test]
fn test_issue5() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(PET_STORE_JSON);
    // Global Tags
    let tags = oas.get_tags();
    assert!(tags.is_some());
    assert_eq!(Some(PET_TAG.to_string()), tags.unwrap().get(0).unwrap().get_name());
    // Operation Tag
    let pet_put = path(&oas, PET_PATH).get_put().unwrap();
    assert!(pet_put.get_tags().is_some());
    assert_eq!(Some(PET_TAG.to_string()), pet_put.get_tags().unwrap().get(0));
}

/// Default value in parameters
#[test]
fn test_issue6() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_6_JSON);
    assert_eq!(integer(DEFAULT_VALUE), params(&get_op(&oas, PETS_PATH))[0].get_schema().unwrap().get_default());
}

/// Body Converted to RequestBody
#[test]
fn test_issue8() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_8_JSON);
    assert!(get_op(&oas, PETS_PATH).get_request_body().is_some());
}

/// Response Headers
#[test]
fn test_issue10() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(PET_STORE_JSON);
    let headers = response(&get_op(&oas, USER_LOGIN_PATH), "200").get_headers().unwrap();
    assert_eq!(Some(X_RATE_LIMIT_DESCRIPTION.to_string()), headers.get(X_RATE_LIMIT).unwrap().get_description());
    assert_eq!(Some(X_EXPIRES_AFTER_DESCRIPTION.to_string()), headers.get(X_EXPIRES_AFTER).unwrap().get_description());
}

/// Minimal Spec error
#[test]
fn test_issue11() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let _oas = get_converted_open_api_from_json_file(ISSUE_11_JSON);
}

/// Extensions in External Docs
#[test]
fn test_issue12() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(PET_STORE_JSON);
    for tag in oas.get_tags().unwrap().to_vec() {
        if let Some(docs) = tag.get_external_docs() {
            assert!(docs.get_extensions().is_none());
        }
    }
}

/// Extensions in External Docs
#[test]
fn test_issue13() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_13_JSON);
    assert!(oas.get_external_docs().is_some());
}

/// X-example in parameters
#[test]
fn test_issue14() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_14_JSON);
    assert_eq!(string(VALUE), params(&get_op(&oas, PETS_PATH))[0].get_example());
}

/// Convert extensions everywhere applicable #15
#[test]
fn test_issue15() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_15_JSON);

    let info = oas.get_info().unwrap();
    assert!(ext(info.get_extensions(), "x-apis-json").is_some());
    assert!(ext(info.get_license().unwrap().get_extensions(), "x-notes").is_some());
    assert!(ext(oas.get_external_docs().unwrap().get_extensions(), "x-docs-extension").is_some());
    let tag = oas.get_tags().unwrap().get(0).unwrap();
    assert!(ext(tag.get_extensions(), "x-tag-extension").is_some());
    assert!(ext(tag.get_external_docs().unwrap().get_extensions(), "x-tag-docs-extension").is_some());

    let path_item = path(&oas, "/something");
    assert!(ext(path_item.get_extensions(), "x-path-item-extension").is_some());

    let get = path_item.get_get().unwrap();
    assert!(ext(get.get_extensions(), "x-version").is_some());
    assert!(ext(get.get_external_docs().unwrap().get_extensions(), "x-operation-docs-extension").is_some());
    assert!(ext(response(&get, "200").get_extensions(), "x-response-extension").is_some());

    let schema = cast(params(&get)[0].get_schema().unwrap(), SchemaKind::Array);
    assert!(ext(schema.get_items().unwrap().get_extensions(), X_EXAMPLE).is_none());

    let security_schemes = oas.get_components().unwrap().get_security_schemes();
    assert!(security_schemes.is_some());
    assert!(ext(security_schemes.unwrap().get("OAuth2Implicit").unwrap().get_extensions(), "x-auth-extension").is_some());
}

/// Security missing
#[test]
fn test_issue16() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_16_JSON);
    assert!(oas.get_security().is_some());
    assert!(oas.get_components().unwrap().get_security_schemes().is_some());
}

/// Referenced parameters are converted incorrectly
#[test]
fn test_issue17() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_17_JSON);
    let request_bodies = oas.get_components().unwrap().get_request_bodies().unwrap();
    let schemas = oas.get_components().unwrap().get_schemas().unwrap();
    assert!(schemas.get("formData_formEmail").is_some());
    assert!(schemas.get("formData_formPassword").is_some());
    assert!(request_bodies.get("bodyParam").unwrap().get_content().unwrap().get("*/*").is_some());
    assert!(post_op(&oas, "/formPost").get_parameters().is_none());
    assert!(post_op(&oas, "/formPost").get_request_body().is_some());
    let schema = get_op(&oas, "/report/{userId}").get_request_body().unwrap().get_content().unwrap().get("multipart/form-data").unwrap().get_schema().unwrap();
    assert!(schema.get_properties().unwrap().get("limitForm").is_some());
}

/// External Docs in Operations
#[test]
fn test_issue18() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_18_JSON);
    assert!(get_op(&oas, PETS_PATH).get_external_docs().is_some());
}

/// Request Body Completions
#[test]
fn test_issue19() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_19_JSON);
    let request_body = post_op(&oas, POST_PATH).get_request_body();
    assert!(request_body.is_some());
    let schema = request_body.unwrap().get_content().unwrap().get(URLENCODED_CONTENT).unwrap().get_schema();
    assert!(schema.is_some());
    let properties = schema.unwrap().get_properties().unwrap();
    assert_eq!(PROPERTIES_SIZE, properties.size());
    let ids = cast(properties.get("ids").unwrap(), SchemaKind::Array);
    assert_eq!(Some(MIN_ITEMS), ids.get_min_items());
    assert_eq!(Some(MAXIMUM), ids.get_max_items());
    // Fixed from Issue 19
    assert_eq!(Some(true), ids.get_unique_items());

    let login = properties.get("login").unwrap();
    assert_eq!(Some(MIN_LENGTH), login.get_min_length());
    assert_eq!(Some(MAX_LENGTH), login.get_max_length());
    assert_eq!(Some(PATTERN.to_string()), login.get_pattern());
    let fav_number = properties.get("favNumber").unwrap();
    assert_eq!(Some(BigDecimal::from_i64(MAXIMUM as i64)), fav_number.get_minimum());
    assert_eq!(Some(BigDecimal::from_i64(MAXIMUM as i64)), fav_number.get_maximum());
    assert_eq!(Some(true), fav_number.get_exclusive_minimum());
    assert_eq!(Some(true), fav_number.get_exclusive_maximum());
    assert_eq!(BigDecimal::from_f64_exact(MULTIPLE_OF_VALUE), BigDecimal::from_f64_exact(fav_number.get_multiple_of().unwrap().to_f64()));

    let day_of_week = properties.get("dayOfWeek").unwrap();
    assert_eq!(string(MONDAY_TUESDAY_VALUE), day_of_week.get_default());
    let an_enum = day_of_week.get_enum().unwrap();
    assert_eq!(string(MONDAY_TUESDAY_VALUE), an_enum.get(0));
    assert_eq!(string(TUESDAY_WEDNESDAY_VALUE), an_enum.get(1));
    assert_eq!(string(WEDNESDAY_THURSDAY_VALUE), an_enum.get(2));
}

/// Response $ref's
#[test]
fn test_issue20() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_20_JSON);
    let responses = oas.get_components().unwrap().get_responses().unwrap();
    assert!(responses.get(RESPONSE_200OK_COMPONENT).is_some());
    assert!(responses.get(DEFAULT_RESPONSE_COMPONENT).is_some());
}

/// File Responses to Binary
#[test]
fn test_issue21() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_21_JSON);
    let schema = response(&get_op(&oas, FILE_PATH), "200").get_content().unwrap().get(APPLICATION_PDF).unwrap().get_schema().unwrap();
    assert_eq!(Some(BINARY_FORMAT.to_string()), schema.get_format());
}

/// Converting Hosts Without Schema
#[test]
fn test_issue22() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let _oas = get_converted_open_api_from_json_file(ISSUE_22_JSON);
}

/// $ref not updated in components (additional properties)
#[test]
fn test_issue23() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_23_JSON);
    let additional_properties = component_schema(&oas, MAP_OBJECTS_MODEL).unwrap().get_additional_properties();
    let Some(AdditionalProperties::Schema(additional_properties)) = additional_properties else {
        panic!("additionalProperties is not a Schema: {additional_properties:?}")
    };
    assert_eq!(Some(OBJECT_REF.to_string()), additional_properties.get_ref());
}

/// Covert path item $refs
#[test]
fn test_issue25() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_25_JSON);
    assert_eq!(path(&oas, "/foo2").get_ref().as_deref(), Some("#/paths/~1foo"));
}

/// Convert allOff
#[test]
fn test_issue26() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_26_JSON);
    let extended_error_model = cast(component_schema(&oas, EXTENDED_ERROR_MODEL).unwrap(), SchemaKind::Composed);
    let all_of = extended_error_model.get_all_of().unwrap();
    assert_eq!(Some(ERROR_MODEL_REF.to_string()), all_of.get(0).unwrap().get_ref());
    assert_eq!(Some(OBJECT.to_string()), all_of.get(1).unwrap().get_type());
}

/// 500 Error Models with discriminator
#[test]
fn test_issue27() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_27_JSON);
    assert_eq!(REQUIRED_SIZE, component_schema(&oas, PET_SCHEMA).unwrap().get_required().unwrap().size());
}

/// OAuth 2 flows and URLs were lost
#[test]
fn test_issue28() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_28_JSON);
    let o_auth2_implicit =
        oas.get_components().unwrap().get_security_schemes().unwrap().get(SECURITY_SCHEMA_OAUTH2).unwrap().get_flows().unwrap().get_implicit().unwrap();
    assert_eq!(Some(AUTHORIZATION_URL.to_string()), o_auth2_implicit.get_authorization_url());
    let scopes = o_auth2_implicit.get_scopes().unwrap();
    assert_eq!(Some(WRITE_PETS_VALUE.to_string()), scopes.get(SCOPE_WRITE_PETS));
    assert_eq!(Some(READ_PETS_VALUE.to_string()), scopes.get(SCOPE_READ_PETS));
}

/// Convert collectionFormat #1 - path, query, header parameters
#[test]
fn test_issue30() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let _oas = get_converted_open_api_from_json_file(ISSUE_30_JSON);
}

/// Expect a default server object when a swagger without host, basePath and scheme is converted to openAPI
#[test]
fn test_issue31() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_31_JSON);
    assert!(oas.get_servers().is_some());
}

/// Convert schema, property and array examples
#[test]
fn test_issue32() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_32_JSON);
    let schemas = oas.get_components().unwrap().get_schemas();
    assert!(schemas.is_some());
    let schemas = schemas.unwrap();
    let properties = schemas.get(USER_MODEL).unwrap().get_properties();
    assert!(properties.is_some());
    let properties = properties.unwrap();
    assert_eq!(properties.get(ID).unwrap().get_example(), integer(EXAMPLE_42_NUMBER));
    assert_eq!(properties.get(NAME).unwrap().get_example(), string(ARTHUR_DENT_NAME));
    let friend_ids = cast(properties.get(FRIEND_IDS).unwrap(), SchemaKind::Array);
    assert_eq!(friend_ids.get_items().unwrap().get_example(), integer(EXAMPLE_8_NUMBER));
    let numbers = JavaObject::List(std::sync::Arc::new(vec![JavaObject::Integer(3), JavaObject::Integer(4), JavaObject::Integer(5)]));
    assert_eq!(friend_ids.get_example(), Some(numbers));
    let example = schemas.get(ARRAY_OF_USERS_MODEL).unwrap().get_example().unwrap_or(JavaObject::Null);
    assert_eq!(json_string(&example).unwrap(), ARRAY_VALUES);
}

/// Convert response examples
#[test]
fn test_issue33() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_33_JSON);
    let api_response = get_op(&oas, USERS_PATH).get_responses().unwrap().get("200");
    assert!(api_response.is_some());
    let content = api_response.unwrap().get_content().unwrap();
    assert!(content.get(APPLICATION_YAML).unwrap().get_example().is_some());
    assert!(content.get(APPLICATION_JSON).is_some());
}

/// Nice to have: Convert x-nullable to nullable
#[test]
fn test_issue35() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_35_JSON);
    let get_operation = path(&oas, FOO_PATH).get_get();
    assert!(get_operation.is_some());
    let get_operation = get_operation.unwrap();
    let parameters = get_operation.get_parameters();
    assert!(parameters.is_some());
    let parameters = parameters.unwrap();
    assert_eq!(parameters.get(0).unwrap().get_schema().unwrap().get_nullable(), Some(true));
    assert_eq!(parameters.get(1).unwrap().get_schema().unwrap().get_nullable(), Some(false));
    assert_eq!(parameters.get(2).unwrap().get_schema().unwrap().get_nullable(), Some(true));
    assert_eq!(response(&get_operation, "200").get_content().unwrap().get("*/*").unwrap().get_schema().unwrap().get_nullable(), Some(true));
    let user = component_schema(&oas, USER_MODEL);
    assert!(user.is_some());
    let user = user.unwrap();
    assert_eq!(user.get_nullable(), Some(true));
    assert_eq!(prop(&user, ID).get_nullable(), Some(true));
}

/// Nice to have: Convert x-example to example
#[test]
fn test_issue36() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_36_JSON);
    let parameters = get_op(&oas, FOO_PATH).get_parameters();
    assert!(parameters.is_some());
    let parameters = parameters.unwrap();
    assert_eq!(parameters.get(0).unwrap().get_example(), string(FOO_VALUE));
    assert_eq!(parameters.get(1).unwrap().get_example(), integer(NUMBER_VALUE_TWENTY));
    assert_eq!(parameters.get(2).unwrap().get_example(), string(CODE_EXAMPLE));

    let request_body = post_op(&oas, LOGIN_PATH).get_request_body();
    assert!(request_body.is_some());
    let properties = request_body.unwrap().get_content().unwrap().get(CONTENT_TYPE).unwrap().get_schema().unwrap().get_properties().unwrap();
    assert_eq!(properties.get(EMAIL).unwrap().get_example(), string(EMAIL_VALUE));
    assert_eq!(properties.get(PASSWORD).unwrap().get_example(), string(PASSWORD_VALUE));
}

/// OpenAPI v2 converter - examples are lost
#[test]
fn test_issue540() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_540_JSON);
    let operation = path(&oas, "/example").get_post();
    assert!(operation.is_some());
    let operation = operation.unwrap();
    let example = operation.get_request_body().unwrap().get_content().unwrap().get(APPLICATION_JSON).unwrap().get_example();
    assert!(example.is_some());
    let example = response(&operation, "200").get_content().unwrap().get(APPLICATION_JSON);
    assert!(example.is_some());
}

/// OpenAPI v2 converter - enum values for array parameters are lost
#[test]
fn test_issue597() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_597_JSON);
    let parameters = get_op(&oas, FOO_PATH).get_parameters();
    assert!(parameters.is_some());
    let an_enum = parameters.unwrap().get(0).unwrap().get_schema().unwrap().get_enum();
    assert!(an_enum.is_some());
    let an_enum = an_enum.unwrap();
    assert_eq!(an_enum.get(0), string(AVAILABLE));
    assert_eq!(an_enum.get(1), string(PENDING));
    assert_eq!(an_enum.get(2), string(SOLD));
}

/// Parser Issue: OpenAPI v2 converter - HEAD and OPTIONS operations are lost
#[test]
fn test_issue599() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_599_JSON);
    let operation = path(&oas, FOO_PATH).get_head();
    assert!(operation.is_some());
    assert_eq!(operation.unwrap().get_description().as_deref(), Some(HEAD_OPERATION));

    let operation = path(&oas, FOO_PATH).get_options();
    assert!(operation.is_some());
    assert_eq!(operation.unwrap().get_description().as_deref(), Some(OPTIONS_OPERATION));
}

/// OpenAPI v2 converter - required Form parameters are converted as optional
#[test]
fn test_issue600() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_600_JSON);

    let request_body = post_op(&oas, LOGIN_PATH).get_request_body();
    assert!(request_body.is_some());
    let request_body = request_body.unwrap();
    assert!(request_body.get_required().expect("unboxing null Boolean"));
    let schema = request_body.get_content().unwrap().get(CONTENT_TYPE).unwrap().get_schema().unwrap();
    let required = schema.get_required();
    assert!(required.is_some());
    assert_eq!(required.unwrap().size(), REQUIRED_SIZE);
}

/// OpenAPI v2 converter - ref in RequestBodies are correctly updated
#[test]
fn test_issue647() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_647_JSON);

    let r#ref = oas.get_components().unwrap().get_request_bodies().unwrap().get("b").unwrap().get_content().unwrap().get("*/*").unwrap().get_schema().unwrap().get_ref();
    assert_eq!(r#ref.as_deref(), Some(SCHEMAS_A_REF));
}

/// OpenAPI v2 converter - NPE when no "paths" is empty
#[test]
fn test_issue662() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_662_JSON);
    assert!(oas.get_paths().unwrap().is_empty());
}

/// OpenAPI v2 converter - NullPointerException when converting an spec #
#[test]
fn test_issue672() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let _oas = get_converted_open_api_from_json_file(ISSUE_672_JSON);
}

/// OpenAPI v2 converter - integer elements of enum are converted to String
#[test]
fn test_issue676() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_676_JSON);
    let parameters = get_op(&oas, FOO_PATH).get_parameters();
    assert!(parameters.is_some());
    let parameters = parameters.unwrap();

    let an_enum = parameters.get(0).unwrap().get_schema().unwrap().get_enum();
    assert!(an_enum.is_some());
    let an_enum = an_enum.unwrap();
    assert_eq!(an_enum.get(0), integer(1));
    assert_eq!(an_enum.get(1), integer(-2));

    let an_enum = parameters.get(1).unwrap().get_schema().unwrap().get_enum();
    assert!(an_enum.is_some());
    let an_enum = an_enum.unwrap();
    let big_decimal = |v: i64| Some(JavaObject::BigDecimal(std::sync::Arc::new(BigDecimal::from_i64(v))));
    assert_eq!(an_enum.get(0), big_decimal(1));
    assert_eq!(an_enum.get(1), big_decimal(-2));

    let an_enum = parameters.get(2).unwrap().get_schema().unwrap().get_enum();
    assert!(an_enum.is_some());
    let an_enum = an_enum.unwrap();
    assert_eq!(an_enum.get(0), Some(JavaObject::Boolean(true)));
    assert_eq!(an_enum.get(1), Some(JavaObject::Boolean(false)));
}

/// OpenAPI v2 converter - Error in BodyParameter convertion
#[test]
fn test_issue673() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_673_YAML);
    let schema = post_op(&oas, "/integer").get_request_body().unwrap().get_content().unwrap().get("application/json").unwrap().get_schema();
    assert!(schema.is_some());
    let schema = schema.unwrap();
    assert!(schema.get_unique_items().expect("unboxing null Boolean"));
    assert!(schema.get_exclusive_maximum().expect("unboxing null Boolean"));
    assert!(schema.get_exclusive_minimum().expect("unboxing null Boolean"));
    // getMultipleOf().toBigInteger().intValue()
    assert_eq!(3, i32::try_from(schema.get_multiple_of().unwrap().to_bigint_truncated()).unwrap());
    assert_eq!(Some(BigDecimal::from_i64(5)), schema.get_minimum());
    assert_eq!(Some(BigDecimal::from_i64(7)), schema.get_maximum());

    let schema = post_op(&oas, "/string").get_request_body().unwrap().get_content().unwrap().get("application/json").unwrap().get_schema().unwrap();
    assert_eq!(2, schema.get_min_length().unwrap());
    assert_eq!(7, schema.get_max_length().unwrap());
    assert_eq!(Some("aaa".to_string()), schema.get_pattern());
}

/// OpenAPI v2 converter - Migrate minLength, maxLength and pattern of String property
#[test]
fn test_issue708() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_708_YAML);
    let schema = component_schema(&oas, "SomeObj");
    assert!(schema.is_some());
    let schema = schema.unwrap();
    assert_eq!(schema.get_min_length(), Some(1));
    assert_eq!(schema.get_max_length(), Some(3));
    assert_eq!(schema.get_pattern().as_deref(), Some("^[0-9]+$"));
}

/// Issue in converting server url in RFC 3986 format from OpenAPI Spec 2 to Open API Spec 3
#[test]
fn test_issue745() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_745_YAML);
    assert!(oas.get_servers().unwrap().get(0).unwrap().get_url().unwrap().starts_with("//"));
}

/// OpenAPIParser.readLocation fails when fetching valid Swagger 2.0 resource with AuthorizationValues provided
#[test]
fn test_issue785() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let api_key = AuthorizationValue::new("api_key", "special-key", "header");
    let authorization_values = vec![api_key];
    let converter = SwaggerConverter::new();
    let converted_auth_list = converter.convert_auths(Some(&authorization_values));
    assert_eq!(converted_auth_list.len(), authorization_values.len());
}

/// OpenAPI v2 converter - Migrate a schema with AllOf
#[test]
fn test_issue740() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_740_YAML);
    let schema = component_schema(&oas, "Action").unwrap();
    assert!(schema.is_composed_schema());
    let composed_schema = schema;
    assert_eq!(composed_schema.get_all_of().unwrap().size(), 2);
}

/// OpenAPI v2 converter - no model in body parameter
#[test]
fn test_issue756() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let _oas = get_converted_open_api_from_json_file(ISSUE_756_JSON);
}

/// OpenAPI v2 converter - NPE when 'enum' field is available and 'type' field is missing in query parameter
#[test]
fn test_issue758() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let _oas = get_converted_open_api_from_json_file(ISSUE_758_JSON);
}

/// OpenAPI v2 Converter: NPE when type is array and 'items' field is missing in array property
#[test]
fn test_issue762() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let _oas = get_converted_open_api_from_json_file(ISSUE_762_JSON);
}

/// OpenAPI v2 Converter: Ensure body name is preserved in x-codegen-request-body-name extension
#[test]
fn test_preserve_body_parameter_name() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_762_JSON);
    assert_eq!(ext(path(&oas, "/").get_put().unwrap().get_extensions(), "x-codegen-request-body-name"), string("pet"));
}

/// requestBody not correctly populated when Parameters is a list of $refs (OAS 2 to 3 conversion)
#[test]
fn test_issue765() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_765_YAML);
    let request_body = post_op(&oas, "/ping/{ActivityTypePath}").get_request_body();
    assert!(request_body.is_some());
    assert_eq!(Some("#/components/requestBodies/Block".to_string()), request_body.unwrap().get_ref());
}

/// OpenAPI v2 converter - Missing Parameter.style property
#[test]
fn test_parameter_conversion() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(PARAMETER_CONVERSION_JSON);
    let parameters = get_op(&oas, FOO_PATH).get_parameters();
    assert!(parameters.is_some());
    let parameters = parameters.unwrap();

    let parameter = parameters.get(0).unwrap();
    assert_eq!(parameter.get_style(), Some(ParameterStyle::Form));
    assert!(!parameter.get_explode().expect("unboxing null Boolean"));

    let parameter = parameters.get(1).unwrap();
    assert_eq!(parameter.get_style(), Some(ParameterStyle::Form));
    assert!(parameter.get_explode().expect("unboxing null Boolean"));

    let parameter = parameters.get(2).unwrap();
    assert_eq!(parameter.get_style(), Some(ParameterStyle::Simple));
    assert!(!parameter.get_explode().expect("unboxing null Boolean"));

    let parameter = parameters.get(3).unwrap();
    assert_eq!(parameter.get_style(), Some(ParameterStyle::Simple));
    assert!(!parameter.get_explode().expect("unboxing null Boolean"));
}

/// OpenAPI v2 converter - SwaggerParseResult has message
#[test]
fn test_swagger_parse_result_has_message() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let swagger_as_string = read_resource(PET_STORE_JSON);
    let parse_options = ParseOptions { resolve: false, ..Default::default() };
    let result = read_contents(&swagger_as_string, Some(&parse_options)).unwrap();

    assert!(result.get_messages().is_some());
}

/// OpenAPI v2 converter - Migrate minLength, maxLength and pattern of String property
#[test]
fn test_issue786() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let _oas = get_converted_open_api_from_json_file(ISSUE_768_JSON);
}

/// OpenAPI v2 converter - Conversion of a spec without a info section
#[test]
fn test_issue755() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let _oas = get_converted_open_api_from_json_file(ISSUE_755_YAML);
}

/// OpenAPI v2 converter - top-level extensions should be preserved
#[test]
fn test_top_level_extensions() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(PARAMETER_CONVERSION_JSON);
    let value = ext(oas.get_extensions(), "x-some-extensions");
    // `(String) value`
    assert!(value.as_ref().is_none_or(|v| matches!(v, JavaObject::String(_))), "ClassCastException");
    assert_eq!(value, string("hello"));
}

/// OpenAPI v2 converter - Conversion param extensions should be preserved
#[test]
fn test_issue820() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_820_YAML);
    let post = post_op(&oas, "/issue820");
    let content = post.get_request_body().unwrap().get_content().unwrap();
    assert!(content.get("multipart/form-data").is_some());
    assert!(content.get("multipart/form-data").unwrap().get_schema().is_some());
    let properties = content.get("multipart/form-data").unwrap().get_schema().unwrap().get_properties();
    assert!(properties.is_some());
    let properties = properties.unwrap();
    assert_eq!(properties.size(), 3, "size");
    let foo = properties.get("foo");
    assert!(foo.is_some());
    let foo = foo.unwrap();
    assert!(foo.get_extensions().is_some());
    assert_eq!(ext(foo.get_extensions(), "x-ext"), string("some foo"));
    assert_eq!(foo.get_nullable(), None);
    let bar = properties.get("bar");
    assert!(bar.is_some());
    let bar = bar.unwrap();
    assert!(bar.get_extensions().is_some());
    assert_eq!(ext(bar.get_extensions(), "x-ext"), string("some bar"));
    assert_eq!(bar.get_nullable(), Some(true));
    let baz = properties.get("baz");
    assert!(baz.is_some());
    let baz = baz.unwrap();
    assert!(baz.get_extensions().is_some());
    assert_eq!(ext(baz.get_extensions(), "x-ext"), string("some baz"));
    assert_eq!(baz.get_nullable(), Some(false));
}

/// OpenAPI v2 converter - proper IntegerSchema parsing
#[test]
fn test_issue1032() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_1032_YAML);
    let unix_timestamp_query_parameter = oas.get_components().unwrap().get_parameters().unwrap().get(UNIX_TIMESTAMP_QUERY_PARAM);
    assert!(unix_timestamp_query_parameter.is_some());
    let s = unix_timestamp_query_parameter.unwrap().get_schema().unwrap();
    assert!(s.is_kind(SchemaKind::Integer), "actual type: {}", s.kind().class_name());
    let integer_schema = s;
    assert_eq!(Some(INTEGER_TYPE.to_string()), integer_schema.get_type());
    assert_eq!(Some(INT64_FORMAT.to_string()), integer_schema.get_format());
}

/// OpenAPI v2 converter - converts URL correctly when it begins with forward slash
#[test]
fn test_issue1113() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_1113_YAML);
    assert!(oas.get_servers().is_some());
    let servers = oas.get_servers().unwrap();
    assert!(!servers.is_empty());
    assert!(servers.get(0).is_some());
    assert_eq!(servers.get(0).unwrap().get_url().as_deref(), Some("/test"));
}

/// OpenAPI v2 converter - uses specialized schema subclasses where available
#[test]
fn test_issue1164() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_1164_YAML);
    assert!(oas.get_paths().is_some());
    assert!(oas.get_paths().unwrap().get("/foo").is_some());
    assert!(path(&oas, "/foo").get_get().is_some());
    assert!(get_op(&oas, "/foo").get_request_body().is_some());
    assert!(get_op(&oas, "/foo").get_request_body().unwrap().get_content().is_some());
    assert!(get_op(&oas, "/foo").get_request_body().unwrap().get_content().unwrap().get("multipart/form-data").is_some());
    let form_schema = get_op(&oas, "/foo").get_request_body().unwrap().get_content().unwrap().get("multipart/form-data").unwrap().get_schema();
    assert!(form_schema.is_some());
    let form_schema = form_schema.unwrap();
    assert!(form_schema.get_properties().is_some());
    let properties = form_schema.get_properties().unwrap();
    assert_eq!(4, properties.size());
    assert!(properties.get("first").unwrap().is_kind(SchemaKind::String));

    assert!(properties.get("second").unwrap().is_kind(SchemaKind::Boolean));

    assert!(properties.get("third").unwrap().is_kind(SchemaKind::String));
    let third = properties.get("third").unwrap();
    assert!(third.get_format().is_some());
    assert_eq!(third.get_format().as_deref(), Some("password"));

    assert!(properties.get("fourth").unwrap().is_kind(SchemaKind::Boolean));
    let fourth = properties.get("fourth").unwrap();
    assert!(fourth.get_type().is_some());
    assert!(fourth.get_format().is_some());
    assert_eq!(fourth.get_format().as_deref(), Some("completely-custom"));
}

/// OpenAPI v2 converter - verifies the references inside composed schema is resolved
#[test]
fn test_issue1261() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_1261_YAML);
    let schema = cast(prop(&component_schema(&oas, "Bar").unwrap(), "bar2"), SchemaKind::Composed);
    assert_eq!(schema.get_all_of().unwrap().get(0).unwrap().get_ref().as_deref(), Some("#/components/schemas/Foo"));
}

/// OpenAPI v2 converter - vendor extensions on body parameters copied to output RequestBody
#[test]
fn test_issue1715() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_1715_YAML);
    let request_body = post_op(&oas, "/foo").get_request_body().unwrap();
    assert!(request_body.get_extensions().is_some());
    assert_eq!(1, request_body.get_extensions().unwrap().size());
    assert_eq!(string("bar"), ext(request_body.get_extensions(), "x-foo"));
}

/// OpenAPI v2 converter - security of operation should be set to empty list if that's the case
#[test]
fn test_issue1767() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_1767_YAML);

    let first_operation_security_requirements = get_op(&oas, "/api/not-secured").get_security();
    assert!(first_operation_security_requirements.is_some());
    assert_eq!(first_operation_security_requirements.unwrap().size(), 0);

    let second_operation_security_requirements = get_op(&oas, "/api/secured/").get_security();
    assert!(second_operation_security_requirements.is_none());
}

/// OpenAPI v2 converter - composed model should keep required properties
#[test]
fn test_issue1796() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file(ISSUE_1796_YAML);
    let schema = cast(component_schema(&oas, "ComposedModel").unwrap(), SchemaKind::Composed);
    assert!(schema.get_required().is_some());
    let required = schema.get_required().unwrap();
    assert_eq!(required.size(), 1);
    assert_eq!(required.get(0).as_deref(), Some("name"));
}

#[test]
fn test_inline_definition_property() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let parse_options = ParseOptions { resolve: true, flatten: true, ..Default::default() };
    let result = read_location("src/test/resources/issue-1359.yaml", Some(&parse_options)).unwrap();
    let oas = result.openapi;
    assert!(oas.is_some());
    let oas = oas.unwrap();

    let pet = component_schema(&oas, "Pet").unwrap();
    let property = prop(&pet, "categoryInline");
    assert_eq!(Some("#/components/schemas/Pet_categoryInline".to_string()), property.get_ref());

    let pet_category_inline = component_schema(&oas, "Pet_categoryInline");
    assert!(pet_category_inline.is_some());
}

#[test]
fn test_convert_form_data_as_object_schema() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let oas = get_converted_open_api_from_json_file("issue-1529.json");

    let companies_schema =
        post_op(&oas, "/companies/").get_request_body().unwrap().get_content().unwrap().get("multipart/form-data").unwrap().get_schema().unwrap();

    assert_eq!(companies_schema.get_type().as_deref(), Some("object"));
    assert_eq!(companies_schema.kind(), SchemaKind::Object);
}

#[test]
fn test_issue2269_shared_response_nullable() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let parse_options = ParseOptions { resolve: true, resolve_fully: true, ..Default::default() };
    let result = read_location(&format!("src/test/resources/{ISSUE_2269_YAML}"), Some(&parse_options));
    assert!(result.is_some());
    let oas = result.unwrap().openapi;
    assert!(oas.is_some());
    let oas = oas.unwrap();

    for endpoint in ["/endpoint1", "/endpoint2"] {
        let content = response(&get_op(&oas, endpoint), "200").get_content().unwrap();
        let properties = content.values().into_iter().next().unwrap().get_schema().unwrap().get_properties();
        assert!(properties.is_some(), "{endpoint} schema properties must not be null");
        let properties = properties.unwrap();

        let nullable_field = properties.get("nullable_field");
        assert!(nullable_field.is_some(), "{endpoint} must have nullable_field");
        let nullable_field = nullable_field.unwrap();
        assert_eq!(nullable_field.get_nullable(), Some(true), "{endpoint} nullable_field must have nullable: true");
        assert!(nullable_field.get_extensions().is_some(), "{endpoint} nullable_field must retain extensions");
        assert_eq!(ext(nullable_field.get_extensions(), "x-internal-note"), string("abc"), "{endpoint} nullable_field must retain x-internal-note extension");

        let non_nullable_field = properties.get("non_nullable_field");
        assert!(non_nullable_field.is_some(), "{endpoint} must have non_nullable_field");
        let non_nullable_field = non_nullable_field.unwrap();
        assert_eq!(non_nullable_field.get_nullable(), Some(false), "{endpoint} non_nullable_field must have nullable: false");
        assert!(non_nullable_field.get_extensions().is_some(), "{endpoint} non_nullable_field must retain extensions");
        assert_eq!(
            ext(non_nullable_field.get_extensions(), "x-internal-note"),
            string("def"),
            "{endpoint} non_nullable_field must retain x-internal-note extension"
        );
    }
}

/// A clone (almost) of `OpenAPIV3Parser#readContents(String, List, ParseOptions)`.
fn read(location: Option<&str>, auths: Option<&[AuthorizationValue]>, resolve: &ParseOptions, parser_extensions: &[&dyn SwaggerParserExtension]) -> Option<OpenAPI> {
    let location = location?;
    for extension in parser_extensions {
        // `parsed.getMessages()` throws a NullPointerException when `readLocation` returned null
        let parsed = extension.try_read_location(location, auths, Some(resolve)).unwrap_or_else(|e| panic!("readLocation threw {e:?}")).expect("NullPointerException: parsed is null");
        if let Some(messages) = parsed.get_messages() {
            for _message in messages {
                // LOGGER.info("{}: {}", extension, message);
            }
        }
        if let Some(result) = parsed.openapi {
            return Some(result);
        }
    }
    None
}

/// Tests an NPE when bother the old and new parser are on the classpath. The only way to test this is cloning the
/// `OpenAPIV3Parser#readContents(String, List, ParseOptions)` to see the NPE when we pass in the extentions.
#[test]
fn test_issue2046() {
    let _g = common::setup(common::CONVERTER_MODULE);
    let options = ParseOptions { resolve: true, ..Default::default() };
    // This should not throw an NPE:
    let open_api = read(Some("I/do/not/exist/on/the/file/system/I/really/do/not.yaml"), None, &options, &[&SwaggerConverter::new()]);
    assert!(open_api.is_none());
}
