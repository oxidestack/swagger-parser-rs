//! Port of `io.swagger.v3.parser.test.OpenAPIV3ParserTest`, part 1 (the tests declared before
//! `testFileReferenceWithVendorExt`).

mod common;

use jackson::JavaObject;
use swagger_parser::core::{AuthorizationValue, ParseOptions, SwaggerParseResult};
use swagger_parser::java::JMap;
use swagger_parser::models::{AdditionalProperties, ApiResponse, OpenAPI, Operation, ParameterKind, PathItem, Schema, SchemaKind};
use swagger_parser::parser::v3_parser::OpenAPIV3Parser;

/// `List<AuthorizationValue> auths = new ArrayList<>()`
fn auths() -> Vec<AuthorizationValue> {
    Vec::new()
}

fn read_location(location: &str, options: &ParseOptions) -> SwaggerParseResult {
    OpenAPIV3Parser::new().read_location(location, None, Some(options))
}

fn read_contents(yaml: &str, options: Option<&ParseOptions>) -> SwaggerParseResult {
    OpenAPIV3Parser::new().read_contents(yaml, None, options)
}

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
fn schema(api: &OpenAPI, name: &str) -> Option<Schema> {
    schemas(api).get(name)
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

fn java_string(o: Option<JavaObject>) -> Option<String> {
    o.map(|o| o.to_java_string())
}

#[test]
fn test_flatten_option_on_dotted_property_names() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { flatten: true, ..Default::default() };
    let parse_result = read_location("dotted-property-names.yaml", &options);
    let open_api = parse_result.openapi.clone();
    assert!(open_api.is_some());
    let open_api = open_api.unwrap();

    if let Some(messages) = &parse_result.messages {
        for message in messages {
            assert!(!message.contains("Could not resolve reference"), "Unexpected resolver error: {message}");
        }
    }

    let schemas = open_api.get_components().unwrap().get_schemas();
    assert!(schemas.is_some());
    let schemas = schemas.unwrap();
    assert!(schemas.get("pathtoapi_object.field").is_some(), "object.field inline object must be extracted");
    assert!(schemas.get("pathtoapi_nodot").is_some(), "nodot items object must be extracted");
    assert!(schemas.get("pathtoapi_objectarray.field").is_some(), "objectarray.field items object must be extracted");

    let body_schema = schemas.get("to_api_body");
    assert!(body_schema.is_some(), "request body schema must be extracted");
    let body_schema = body_schema.unwrap();

    let string_field = prop(&body_schema, "string.field");
    assert!(string_field.is_some(), "string.field must be present in the body schema");
    let string_field = string_field.unwrap();
    assert!(string_field.get_ref().is_none(), "string.field must not be replaced with a $ref");
    assert_eq!(string_field.get_type().as_deref(), Some("string"), "string.field must remain a string type");

    let object_field_ref = prop(&body_schema, "object.field").unwrap().get_ref();
    assert!(object_field_ref.is_some(), "object.field must have a $ref");
    let object_field_ref = object_field_ref.unwrap();
    assert!(object_field_ref.starts_with("#/components/schemas/"), "object.field $ref must be a valid JSON Pointer, was: {object_field_ref}");

    let nodot = prop(&body_schema, "nodot").unwrap();
    assert!(nodot.is_array_schema());
    let nodot_ref = nodot.get_items().unwrap().get_ref();
    assert!(nodot_ref.is_some(), "nodot items must have a $ref");
    let nodot_ref = nodot_ref.unwrap();
    assert!(nodot_ref.starts_with("#/components/schemas/"), "nodot items $ref must be a valid JSON Pointer, was: {nodot_ref}");

    let dotted = prop(&body_schema, "objectarray.field").unwrap();
    assert!(dotted.is_array_schema());
    let dotted_ref = dotted.get_items().unwrap().get_ref();
    assert!(dotted_ref.is_some(), "objectarray.field items must have a $ref");
    let dotted_ref = dotted_ref.unwrap();
    assert!(dotted_ref.starts_with("#/components/schemas/"), "objectarray.field items $ref must be a valid JSON Pointer, was: {dotted_ref}");
}

/// Issue 2223: reading 3.1 spec from Windows file path location produces URISyntaxException
#[test]
fn test_windows_file_path_read() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPIV3Parser::new().read("\\issue-2223\\openapi.yaml");
    assert!(open_api.is_some(), "OpenAPI object should not be null (no errors during read())");
}

#[test]
fn test_failed_to_resolve_response_references() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, ..Default::default() };
    let open_api = api(&read_location("issue-2037/openapi.yaml", &options));
    assert!(path(&open_api, "/get").get_ref().is_none());
    let get = path(&open_api, "/get").get_get().unwrap();
    assert_eq!(content_schema(&response(&get, "200"), "application/json").get_ref().as_deref(), Some("#/components/schemas/ResponsesRef"));
}

#[test]
fn test_failed_to_resolve_external_references() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, ..Default::default() };
    let open_api = api(&read_location("resolve-external-ref/failedToResolveExternalRefs.yaml", &options));
    const MT: &str = "application/vnd.api+json";
    assert!(path(&open_api, "/permAssignments").get_ref().is_none());
    let op = path(&open_api, "/permAssignments").get_get().unwrap();
    assert_eq!(content_schema(&response(&op, "202"), MT).get_ref().as_deref(), Some("#/components/schemas/schemaResponseSuccess"));
    assert!(path(&open_api, "/permAssignmentChangeRequests").get_ref().is_none());
    let op = path(&open_api, "/permAssignmentChangeRequests").get_get().unwrap();
    assert_eq!(content_schema(&response(&op, "202"), MT).get_ref().as_deref(), Some("#/components/schemas/schemaResponseSuccess"));
    assert!(path(&open_api, "/permAssignmentChange").get_ref().is_none());
    let op = path(&open_api, "/permAssignmentChange").get_get().unwrap();
    assert_eq!(content_schema(&response(&op, "201"), MT).get_ref().as_deref(), Some("#/components/schemas/Error"));
    assert_eq!(content_schema(&response(&op, "404"), MT).get_ref().as_deref(), Some("#/components/schemas/RemoteError"));
}

#[test]
fn test_issue_dereferencing_composed_schema_one_of() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, ..Default::default() };
    let open_api = api(&read_location("resolverIssue/Beta-1-swagger.yaml", &options));
    assert!(schema(&open_api, "naNumber").is_some());
    assert!(schema(&open_api, "unNumber").is_some());
}

#[test]
fn test_issue1621() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, resolve_fully: true, resolve_combinators: false, ..Default::default() };
    let parse_result = read_location("issue-1621/example.openapi.yaml", &options);
    assert_eq!(messages(&parse_result).len(), 0);
    let open_api = api(&parse_result);
    let post = path(&open_api, "/example").get_post().unwrap();
    let title = post.get_request_body().unwrap().get_content().unwrap().get("application/json").unwrap().get_schema().unwrap().get_title();
    assert_eq!(title.as_deref(), Some("POST Example"));
}

#[test]
fn test_issue1865() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, ..Default::default() };
    let result = read_location("src/test/resources/issue-1865/openapi30.yaml", &options);
    assert!(result.openapi.is_some());
    assert_eq!(schema(&api(&result), "Foo").unwrap().get_ref().as_deref(), Some("#/components/schemas/foomodel"));
}

#[test]
fn test_issue1777() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve_fully: true, ..Default::default() };
    let open_api = api(&read_location("issue-1777/issue1777.yaml", &options));
    let id = prop(&schema(&open_api, "customer").unwrap(), "id").unwrap();
    assert_eq!(id.get_type().as_deref(), Some("string"));
    assert_eq!(id.get_format().as_deref(), Some("uuid"));
    assert_eq!(id.get_description().as_deref(), Some("The id of the customer"));
    assert!(id.get_extensions().unwrap().get("x-apigen-mapping").is_some());
    assert_eq!(id.get_min_length(), Some(36));
    assert_eq!(id.get_max_length(), Some(36));
    assert!(id.get_pattern().is_some());
}

#[test]
fn test_issue1802() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve_fully: true, ..Default::default() };
    let open_api = api(&read_location("issue-1802/issue1802.yaml", &options));
    let props = schema(&open_api, "standard_response_res_one").unwrap().get_properties().unwrap();
    assert!(props.get("data").is_some());
    assert!(props.get("result").is_some());
}

#[test]
fn test_issue1780() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { infer_schema_type: false, ..Default::default() };
    let default_schema_type = "---\n\
openapi: '3.0'\n\
info:\n\
\x20 description: This is a credit card account service tier 3 microservice\n\
\x20 title: SC-CCS-CCSRV_AC Credit Card Account Service\n\
\x20 version: 1.0.0\n\
tags:\n\
- name: creditCardAccount\n\
\x20 description: Credit Card Accounts\n\
paths: {}\n\
x-ibm-configuration: {}";
    let result = read_contents(default_schema_type, Some(&options));
    assert!(result.openapi.is_some());
    assert!(messages(&result).contains(&"The provided definition does not specify a valid version field".to_string()));
}

#[test]
fn test_parameters_and_responses_as_numbers() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, resolve_responses: true, ..Default::default() };
    let result = read_location("src/test/resources/parametersAsNumbers/swagger.yaml", &options);
    assert!(result.openapi.is_some());
    let get = path(&api(&result), "/api/deal/{dealId}").get_get().unwrap();
    assert_eq!(get.get_parameters().unwrap().get(0).unwrap().get_name().as_deref(), Some("dealId"));
    assert_eq!(response(&get, "200").get_description().as_deref(), Some("Success"));
}

#[test]
fn test_issue1758() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions::default();
    let result = read_location("src/test/resources/issue1758.yaml", &options);
    assert!(result.openapi.is_some());
    let m = messages(&result);
    assert_eq!(m.len(), 9);
    for expected in [
        "paths.'/path1'.$ref target #/components/schemas/xFoo is not of expected type PathItem",
        "paths.'/foo'(get).parameters.$ref target #/components/schemas/xFoo is not of expected type Parameter/Header",
        "paths.'/foo'(get).responses.default.headers.three.$ref target #/components/schemas/xFoo is not of expected type Header/Parameter",
        "paths.'/foo'(get).requestBody.$ref target #/components/schemas/xFoo is not of expected type RequestBody",
        "paths.'/foo'(get).responses.200.links.user.$ref target #/components/schemas/xFoo is not of expected type Link",
        "paths.'/foo'(get).responses.200.content.'application/json'.schema.$ref target #/components/parameters/pet is not of expected type Schema",
        "paths.'/foo'(get).responses.200.content.'application/json'.examples.one.$ref target #/components/schemas/xFoo is not of expected type Examples",
        "paths.'/foo'(get).responses.400.$ref target #/components/schemas/xFoo is not of expected type Response",
        "paths.'/foo'(get).callbacks.$ref target #/components/schemas/xFoo is not of expected type Callback",
    ] {
        assert!(m.contains(&expected.to_string()), "{expected}");
    }
}

/// Test for not setting the schema type as default
#[test]
fn test_not_default_schema_type() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { infer_schema_type: false, ..Default::default() };
    let default_schema_type = "openapi: 3.0.0\n\
info:\n\
\x20 title: ping test\n\
\x20 version: '1.0'\n\
servers:\n\
\x20 - url: 'http://localhost:8000/'\n\
paths:\n\
\x20 /ping:\n\
\x20   get:\n\
\x20     operationId: pingGet\n\
\x20     responses:\n\
\x20       '201':\n\
\x20         description: OK\n\
components:\n\
\x20 schemas:\n\
\x20   AnyValueModelInline:\n\
\x20     description: test any value inline\n\
\x20     type: object\n\
\x20     properties:\n\
\x20       array_any_value:\n\
\x20         items: {}\n\
\x20       map_any_value:\n\
\x20        additionalProperties: {}\n\
\x20       any_value: {}\n\
\x20       any_value_with_desc:\n\
\x20         description: inline any value\n\
\x20       any_value_nullable:\n\
\x20         nullable: true\n\
\x20         description: inline any value nullable\n\
\x20       map_any_value_with_desc:\n\
\x20         additionalProperties: \n\
\x20           description: inline any value\n\
\x20       map_any_value_nullable:\n\
\x20         additionalProperties:\n\
\x20           nullable: true\n\
\x20           description: inline any value nullable\n\
\x20       array_any_value_with_desc:\n\
\x20         items: \n\
\x20           description: inline any value\n\
\x20       array_any_value_nullable:\n\
\x20         items:\n\
\x20           nullable: true\n\
\x20           description: inline any value nullable";
    let result = read_contents(default_schema_type, Some(&options));
    let open_api = result.openapi.clone();
    assert!(open_api.is_some());
    let model = schema(&open_api.unwrap(), "AnyValueModelInline").unwrap();

    // map_any_value as object when it should be null (every property is a Schema)
    let s = prop(&model, "map_any_value");
    assert!(s.is_some());
    assert!(s.unwrap().get_type().is_none());

    let s = prop(&model, "map_any_value_with_desc");
    assert!(s.is_some());
    assert!(s.unwrap().get_type().is_none());

    let s = prop(&model, "map_any_value_nullable");
    assert!(s.is_some());
    assert!(s.unwrap().get_type().is_none());

    let s = prop(&model, "array_any_value");
    assert!(s.is_some());
    let s = s.unwrap();
    assert!(s.get_items().is_some());
    assert!(s.get_type().is_none());

    let s = prop(&model, "array_any_value_with_desc");
    assert!(s.is_some());
    let s = s.unwrap();
    assert!(s.get_items().unwrap().get_description().is_some());
    assert!(s.get_type().is_none());

    let s = prop(&model, "array_any_value_nullable");
    assert!(s.is_some());
    let s = s.unwrap();
    assert!(s.get_type().is_none());
    assert!(s.get_items().is_some());
    assert!(s.get_items().unwrap().get_description().is_some());
    assert!(s.get_items().unwrap().get_nullable().unwrap());
}

#[test]
fn test_issue1637_style_and_content() {
    let _g = common::setup(common::V3_MODULE);
    let result = read_location("src/test/resources/issue1637.yaml", &ParseOptions::default());
    let param = path(&api(&result), "/test").get_get().unwrap().get_parameters().unwrap().get(0).unwrap();
    assert!(param.get_style().is_none());
    assert!(param.get_explode().is_none());
    assert!(param.get_content().is_some());
}

const ISSUE_1643: &str = "openapi: \"3.0.0\"\n\
info:\n\
\x20 version: 1.0.0\n\
\x20 title: People\n\
paths:\n\
\x20 /person:\n\
\x20   get:\n\
\x20     operationId: getPerson\n\
\x20     parameters:\n\
\x20       - name: name\n\
\x20         in: query\n\
\x20         required: true\n\
\x20         schema:\n\
\x20           type: string\n\
\x20     responses:\n\
\x20       '200':\n\
\x20         description: The person with the given name.          \n\
\x20         content:\n\
\x20           application/json:\n\
\x20             schema:\n\
\x20               $ref: \"#/components/schemas/Person\"\n\
components:\n\
\x20 schemas:\n\
\x20   Person:\n\
\x20     required:\n\
\x20     - name\n\
\x20     type: object\n\
\x20     properties:\n\
\x20       name:\n\
\x20         type: string\n\
\x20       employee:\n\
\x20         $ref: \"#/components/schemas/Employee\"";

#[test]
fn test_issue1643_true() {
    let _g = common::setup(common::V3_MODULE);
    let result = read_contents(ISSUE_1643, Some(&ParseOptions::default()));
    assert!(result.openapi.is_some());
    assert_eq!(messages(&result).len(), 1);
    assert!(messages(&result).contains(&"attribute components.schemas.Person.Employee is not of type `schema`".to_string()));
}

#[test]
fn test_issue1643_false() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { validate_internal_refs: false, ..Default::default() };
    let result = read_contents(ISSUE_1643, Some(&options));
    assert!(result.openapi.is_some());
    assert_eq!(messages(&result).len(), 0);
    assert!(!messages(&result).contains(&"attribute components.schemas.Person.Employee is not of type `schema`".to_string()));
}

/// Restores `System.clearProperty(SchemaTypeUtil.BINARY_AS_STRING)` even when an assertion fails.
struct ClearBinaryAsString;

impl Drop for ClearBinaryAsString {
    fn drop(&mut self) {
        swagger_parser::java::system::clear_property(swagger_parser::parser::deserializer::schema::BINARY_AS_STRING);
    }
}

#[test]
fn test_example_format_byte() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions::default();
    let result = read_location("src/test/resources/issue1630.yaml", &options);
    assert!(result.openapi.is_some());
    let model = prop(&schema(&api(&result), "Response").unwrap(), "content").unwrap();
    assert_eq!(model.kind(), SchemaKind::ByteArray);
    let Some(JavaObject::Bytes(bytes)) = model.get_example() else { panic!("byte[] example expected: {:?}", model.get_example()) };
    assert_eq!(String::from_utf8_lossy(&bytes), "VGhpc1Nob3VsZFBhc3MK");

    let _clear = ClearBinaryAsString;
    swagger_parser::java::system::set_property(swagger_parser::parser::deserializer::schema::BINARY_AS_STRING, "true");
    let result = read_location("src/test/resources/issue1630.yaml", &options);
    assert!(result.openapi.is_some());
    let model = prop(&schema(&api(&result), "Response").unwrap(), "content").unwrap();
    assert_eq!(model.kind(), SchemaKind::String);
    assert!(matches!(model.get_example(), Some(JavaObject::String(s)) if &*s == "VGhpc1Nob3VsZFBhc3MK"));
}

#[test]
fn test_issue1658() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, ..Default::default() };
    let result = read_location("src/test/resources/issue-1658/issue1658.yaml", &options);
    assert!(result.openapi.is_some());
    let open_api = api(&result);
    assert_eq!(messages(&result).len(), 0);
    assert!(!messages(&result).contains(&"String index out of range: -1".to_string()));
    assert_eq!(schema(&open_api, "ref1").unwrap().get_ref().as_deref(), Some("#/components/schemas/ref2"));
    assert!(schema(&open_api, "ref2").is_some());
    assert!(schema(&open_api, "ref2").unwrap().get_ref().is_none());
}

#[test]
fn test_issue1644_null_value() {
    let _g = common::setup(common::V3_MODULE);
    let issue1644 = "openapi: 3.0.0\n\
info:\n\
\x20 title: Operations\n\
\x20 version: 0.0.0\n\
paths:\n\
\x20 \"/operations\":\n\
\x20   post:\n\
\x20     parameters:\n\
\x20       - name: param0\n\
\x20         schema:\n\
\x20           type: string\n\
\x20     responses:\n\
\x20       default:\n\
\x20         description: None\n";
    let result = read_contents(issue1644, Some(&ParseOptions::default()));
    assert!(result.openapi.is_some());
    let m = messages(&result);
    assert_eq!(m.len(), 1);
    assert!(m.contains(&"attribute paths.'/operations'(post).parameters.[param0].in is missing".to_string()));
    assert!(!m.contains(&"attribute paths.'/operations'(post).parameters.[param0].in is not of type `string`".to_string()));
}

#[test]
fn test_issue1644_empty_value() {
    let _g = common::setup(common::V3_MODULE);
    let issue1644 = "openapi: 3.0.0\n\
info:\n\
\x20 title: Operations\n\
\x20 version: 0.0.0\n\
paths:\n\
\x20 \"/operations\":\n\
\x20   post:\n\
\x20     parameters:\n\
\x20       - name: param0\n\
\x20         in: ''\n\
\x20         schema:\n\
\x20           type: string\n\
\x20     responses:\n\
\x20       default:\n\
\x20         description: None\n";
    let result = read_contents(issue1644, Some(&ParseOptions::default()));
    assert!(result.openapi.is_some());
    let m = messages(&result);
    assert_eq!(m.len(), 1);
    assert!(m.contains(&"attribute paths.'/operations'(post).parameters.[param0].in is not of type `[query|header|path|cookie]`".to_string()));
}

#[test]
fn test_empty_strings_false() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { allow_empty_strings: false, ..Default::default() };
    let result = read_location("src/test/resources/empty-strings.yaml", &options);
    assert!(result.openapi.is_some());
    let open_api = api(&result);
    let info = open_api.get_info().unwrap();
    assert!(info.get_title().is_none());
    assert!(info.get_version().is_some());
    assert!(info.get_license().unwrap().get_name().is_none());
    let get = path(&open_api, "/something").get_get().unwrap();
    assert!(response(&get, "200").get_description().is_none());
    assert!(get.get_parameters().unwrap().get(0).unwrap().get_description().is_none());
}

#[test]
fn test_empty_strings_true() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { allow_empty_strings: true, ..Default::default() };
    let result = read_location("src/test/resources/empty-strings.yaml", &options);
    assert!(result.openapi.is_some());
    let open_api = api(&result);
    let info = open_api.get_info().unwrap();
    assert!(info.get_title().is_some());
    assert!(info.get_license().unwrap().get_name().is_some());
    let get = path(&open_api, "/something").get_get().unwrap();
    assert!(response(&get, "200").get_description().is_some());
    assert!(get.get_parameters().unwrap().get(0).unwrap().get_description().is_some());
}

#[test]
fn test_issue1561() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, ..Default::default() };
    let open_api = api(&read_location("issue-1561/swagger.yaml", &options));
    assert_eq!(open_api.get_components().unwrap().get_responses().unwrap().len(), 3);
}

#[test]
fn test_anonymous_model_all_of() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve_fully: true, ..Default::default() };
    let open_api = api(&read_location("issue203/issue203AllOf.yaml", &options));
    assert_eq!(schema(&open_api, "Supplier").unwrap().get_xml().unwrap().get_name().as_deref(), Some("supplierObject"));
}

#[test]
fn test_issue1518() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, ..Default::default() };
    let open_api = api(&read_location("issue-1518/api.json", &options));
    assert_eq!(prop(&schema(&open_api, "Analemmata").unwrap(), "tashotSipe").unwrap().get_ref().as_deref(), Some("#/components/schemas/TashotSipe"));
    assert!(schema(&open_api, "analemmata").is_none());
    assert!(schema(&open_api, "TashotSipe_1").is_none());
    assert!(schema(&open_api, "Stunts").is_some());
    assert!(schema(&open_api, "Stunts_1").is_none());
}

#[test]
fn test_issue1518_stack_over_flow() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, ..Default::default() };
    // A StackOverflowError fails the Java test; in Rust a stack overflow aborts the test binary.
    let result = read_location("example0/api.json", &options);
    // Yaml.prettyPrint(schemas): prints the YAML (or the exception's stack trace) and swallows errors
    let _ = swagger_parser::models::yaml_pretty(&schemas(&api(&result)));
}

#[test]
fn test_cant_read_deep_properties() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve_fully: true, validate_internal_refs: false, ..Default::default() };
    let parse_result = read_location("src/test/resources/cant-read-deep-properties.yaml", &options);
    assert_eq!(messages(&parse_result).len(), 0);
    let projects = prop(&schema(&api(&parse_result), "Project").unwrap(), "project_type").unwrap();
    assert_eq!(projects.get_type().as_deref(), Some("integer"));
}

#[test]
fn test_issue_same_refs_different_model_valid() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, resolve_fully: true, ..Default::default() };
    let result = read_location("src/test/resources/same-refs-different-model-valid.yaml", &options);
    assert_eq!(messages(&result).len(), 0);
}

#[test]
fn test_issue1398() {
    let _g = common::setup(common::V3_MODULE);
    let result = read_location("issue1398.yaml", &ParseOptions::default());
    assert_eq!(messages(&result)[0], "paths.'/pet/{petId}'(get).parameters.[petId].schemas.multipleOf value must be > 0");
}

fn full_flatten_options() -> ParseOptions {
    ParseOptions { resolve: true, resolve_combinators: true, resolve_fully: true, flatten: true, ..Default::default() }
}

#[test]
fn test_issue1367() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = api(&read_location("issue-1367.yaml", &full_flatten_options()));
    assert!(prop(&schema(&open_api, "TestDTO").unwrap(), "choice").unwrap().get_enum().is_some());
}

#[test]
fn test_deserialize_example_flag() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = api(&read_location("exampleFlag.yaml", &full_flatten_options()));
    for name in ["TestDTO", "TestString", "TestNumber"] {
        let s = schema(&open_api, name).unwrap();
        assert!(s.get_example_set_flag(), "{name}");
        assert!(s.get_example().is_none(), "{name}");
    }
    for name in ["TestDTOMissing", "TestStringMissing", "TestNumberMissing"] {
        let s = schema(&open_api, name).unwrap();
        assert!(!s.get_example_set_flag(), "{name}");
        assert!(s.get_example().is_none(), "{name}");
    }
}

#[test]
fn test_example_flag() {
    let _g = common::setup(common::V3_MODULE);
    let parse_result = read_location("media-type-null-example.yaml", &full_flatten_options());
    let open_api = api(&parse_result);

    let mt = path(&open_api, "/pets/{petId}").get_get().unwrap().get_responses().unwrap().get("200").unwrap().get_content().unwrap().get("application/json").unwrap();
    assert!(mt.get_example().is_none());
    assert!(mt.get_example_set_flag());

    let post = path(&open_api, "/pet").get_post().unwrap();
    let mt = response(&post, "200").get_content().unwrap().get("application/json").unwrap();
    assert!(mt.get_example().is_none());
    assert!(!mt.get_example_set_flag());

    let mt = post.get_request_body().unwrap().get_content().unwrap().get("application/json").unwrap();
    assert!(mt.get_example().is_some());
    assert!(mt.get_example().is_some());
    assert!(mt.get_example_set_flag());

    let examples = response(&path(&open_api, "/object-with-null-example").get_get().unwrap(), "200").get_content().unwrap().get("application/json").unwrap().get_examples().unwrap();
    assert!(examples.get("foo").unwrap().get_value().is_some());
    assert!(examples.get("foo").unwrap().get_value_set_flag());
    assert!(examples.get("bar").unwrap().get_value().is_none());
    assert!(examples.get("bar").unwrap().get_value_set_flag());

    let examples = response(&path(&open_api, "/object-with-null-in-schema-example").get_get().unwrap(), "200").get_content().unwrap().get("application/json").unwrap().get_examples().unwrap();
    for name in ["a", "b", "c"] {
        assert!(examples.get(name).unwrap().get_value().is_some(), "{name}");
        assert!(examples.get(name).unwrap().get_value_set_flag(), "{name}");
    }
    assert!(examples.get("d").unwrap().get_value().is_none());
    assert!(examples.get("d").unwrap().get_value_set_flag());

    let s = schema(&open_api, "ObjectWithNullExample").unwrap();
    assert!(s.get_example().is_none());
    assert!(s.get_example_set_flag());

    let s = schema(&open_api, "ObjectWithNullInSchemaExample").unwrap();
    assert!(s.get_example().is_some());
    assert!(s.get_example_set_flag());

    let s = schema(&open_api, "ObjectWithNullPropertyExample").unwrap();
    assert!(prop(&s, "a").unwrap().get_example().is_some());
    assert!(prop(&s, "a").unwrap().get_example_set_flag());
    assert!(prop(&s, "b").unwrap().get_example().is_none());
    assert!(prop(&s, "b").unwrap().get_example_set_flag());

    let s = schema(&open_api, "StringWithNullExample").unwrap();
    assert!(s.get_example().is_none());
    assert!(s.get_example_set_flag());

    let s = schema(&open_api, "ArrayWithNullArrayExample").unwrap();
    assert!(s.get_example().is_none());
    assert!(s.get_example_set_flag());

    let s = schema(&open_api, "ArrayWithNullItemExample").unwrap();
    assert!(s.is_array_schema());
    assert!(s.get_items().unwrap().get_example().is_none());
    assert!(s.get_items().unwrap().get_example_set_flag());
}

fn flatten_options(composed: bool, camel_case: bool) -> ParseOptions {
    ParseOptions { resolve: true, flatten: true, flatten_composed_schemas: composed, camel_case_flatten_naming: camel_case, ..Default::default() }
}

/// `((ComposedSchema) schema).getXxxOf().get(i).get$ref()`
fn composed_ref(s: Option<Schema>, list: &str, i: usize) -> Option<String> {
    let s = s.expect("schema");
    assert!(s.is_composed_schema(), "ComposedSchema expected, got {}", s.kind().simple_name());
    let items = match list {
        "allOf" => s.get_all_of(),
        "anyOf" => s.get_any_of(),
        "oneOf" => s.get_one_of(),
        _ => unreachable!(),
    };
    items.unwrap().get(i).unwrap().get_ref()
}

#[test]
fn test_issue_flatten_additional_properties_schema_inline_model_true() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = api(&read_location("additionalPropertiesFlatten.yaml", &flatten_options(true, true)));
    // responses
    assert!(schema(&open_api, "InlineResponseMap200").is_some());
    assert_eq!(composed_ref(schema(&open_api, "InlineResponseMap200"), "oneOf", 0).as_deref(), Some("#/components/schemas/Macaw1"));
    assert!(schema(&open_api, "InlineResponseMapItems404").is_some());
    assert_eq!(composed_ref(schema(&open_api, "InlineResponseMapItems404"), "anyOf", 0).as_deref(), Some("#/components/schemas/Macaw2"));
}

#[test]
fn test_issue_flatten_array_schema_items_inline_model_false() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = api(&read_location("flattenArrayItems.yaml", &flatten_options(false, false)));
    for name in ["Inline_response_items200", "Inline_response_400", "Inline_parameter_items_bodylimit", "Pagelimit", "Body", "Inline_response_items200", "Inline_array_items_ArrayTest"] {
        assert!(schema(&open_api, name).is_none(), "{name}");
    }
}

#[test]
fn test_issue_flatten_array_schema_items_inline_model_true() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = api(&read_location("flattenArrayItems.yaml", &flatten_options(true, true)));
    // responses
    assert!(schema(&open_api, "InlineResponseItems200").is_some());
    assert_eq!(composed_ref(schema(&open_api, "InlineResponseItems200"), "anyOf", 0).as_deref(), Some("#/components/schemas/Macaw"));
    assert!(schema(&open_api, "InlineResponse400").is_some());
    assert_eq!(composed_ref(schema(&open_api, "InlineResponse400"), "anyOf", 0).as_deref(), Some("#/components/schemas/Macaw3"));
    // parameters
    assert!(schema(&open_api, "InlineParameterItemsBodylimit").is_some());
    assert_eq!(composed_ref(schema(&open_api, "InlineParameterItemsBodylimit"), "anyOf", 0).as_deref(), Some("#/components/schemas/Macaw1"));
    assert!(schema(&open_api, "Pagelimit").is_some());
    assert_eq!(composed_ref(schema(&open_api, "Pagelimit"), "oneOf", 0).as_deref(), Some("#/components/schemas/Macaw2"));
    // requestBodies
    assert!(schema(&open_api, "RequestBodiesBody").is_some());
    assert_eq!(composed_ref(schema(&open_api, "RequestBodiesBody"), "allOf", 1).as_deref(), Some("#/components/schemas/requestBodiesAllOf2"));
    assert!(schema(&open_api, "InlineResponseItems200").is_some());
    assert_eq!(composed_ref(schema(&open_api, "InlineBodyItemsApplicationxmlrequestBodies"), "allOf", 1).as_deref(), Some("#/components/schemas/ApplicationxmlAllOf2"));
    // components
    assert!(schema(&open_api, "InlineArrayItemsArrayTest").is_some());
    assert_eq!(composed_ref(schema(&open_api, "InlineArrayItemsArrayTest"), "oneOf", 1).as_deref(), Some("#/components/schemas/ArrayTestOneOf2"));
}

#[test]
fn test_camel_case_flatten_naming_false() {
    let _g = common::setup(common::V3_MODULE);
    let result = read_location("FlattenComposedSchemasAtComponents.yaml", &flatten_options(true, false));
    assert!(result.openapi.is_some());
    let open_api = api(&result);
    assert_eq!(composed_ref(schema(&open_api, "contact-base-model"), "allOf", 0).as_deref(), Some("#/components/schemas/contactbasemodelAllOf_1"));
}

#[test]
fn test_issue_flatten_composed_schema_inline_model() {
    let _g = common::setup(common::V3_MODULE);
    let result = read_location("FlattenComposedSchemasAtComponents.yaml", &flatten_options(true, true));
    assert!(result.openapi.is_some());
    let open_api = api(&result);
    assert_eq!(composed_ref(schema(&open_api, "contact-base-model"), "allOf", 0).as_deref(), Some("#/components/schemas/ContactBaseModelAllOf1"));
    assert_eq!(composed_ref(schema(&open_api, "Test"), "oneOf", 1).as_deref(), Some("#/components/schemas/TestOneOf2"));
}

#[test]
fn test_issue_flatten_composed_schema_inline_model_flag_false() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, flatten: true, flatten_composed_schemas: false, ..Default::default() };
    let result = read_location("FlattenComposedSchemasAtComponents.yaml", &options);
    assert!(result.openapi.is_some());
    let open_api = api(&result);
    assert!(composed_ref(schema(&open_api, "contact-base-model"), "allOf", 0).is_none());
    assert!(composed_ref(schema(&open_api, "Test"), "oneOf", 1).is_none());
}

#[test]
fn test_issue1309() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, ..Default::default() };
    let parse_result = read_location("issue-1309.yaml", &options);
    assert!(parse_result.openapi.is_some());
    assert_eq!(messages(&parse_result)[0], "attribute components.schemas.customer-not-found.examples is unexpected");
}

#[test]
fn test_issue1316() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, ..Default::default() };
    let parse_result = read_location("Issue1316.yaml", &options);
    assert!(parse_result.openapi.is_some());
    assert_eq!(messages(&parse_result).len(), 0);
}

#[test]
fn test_issue_1292() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, ..Default::default() };
    let open_api = api(&read_location("issue-1292/petstore.yml", &options));
    let get = path(&open_api, "/pets").get_get().unwrap();
    // assertNotNull(value, message): only the value is checked
    assert!(content_schema(&response(&get, "200"), "application/json").get_ref().is_some());
    assert!(content_schema(&get.get_responses().unwrap().get_default().unwrap(), "application/json").get_ref().is_some());
    assert!(schema(&open_api, "Pet").is_some());
    assert!(schema(&open_api, "Pets").is_some());
    assert!(schema(&open_api, "Error").is_some());
}

#[test]
fn test_issue_505() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, ..Default::default() };
    let open_api = api(&read_location("issue-505/petstore.yml", &options));
    let components = open_api.get_components().unwrap();
    assert!(schema(&open_api, "DateWithExample").is_some());
    assert!(components.get_examples().unwrap().get("DateWithExample").is_some());
    assert!(components.get_links().unwrap().get("userRepository").is_some());
    assert_eq!(path(&open_api, "/pets").get_get().unwrap().get_parameters().unwrap().len(), 3);
}

#[test]
fn test_flatten_composed_schema() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, flatten: true, flatten_composed_schemas: true, ..Default::default() };
    let open_api = api(&read_location("flattenComposedSchemaComplete.json", &options));
    assert!(schema(&open_api, "val_Members_val_member").is_some());
    assert!(schema(&open_api, "val_MemberProducts_val_product").is_some());
}

#[test]
fn test_not_flatten_composed_schema() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, flatten: true, ..Default::default() };
    let open_api = api(&read_location("flattenComposedSchemaComplete.json", &options));
    assert!(schema(&open_api, "val_Members_val_member").is_none());
    assert!(schema(&open_api, "val_MemberProducts_val_product").is_some());
}

#[test]
fn test_codegen_issue8601() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, flatten: true, ..Default::default() };
    let open_api = api(&read_location("codegen-issue-8601.yaml", &options));
    for name in ["status", "test_body", "inline_response_200", "test_body_1", "Test1", "Test2"] {
        assert!(schema(&open_api, name).is_some(), "{name}");
    }
}

#[test]
fn test_codegen_issue9773() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, flatten: true, ..Default::default() };
    let open_api = api(&read_location("codegen-issue-9773/openapi.yaml", &options));
    assert!(schema(&open_api, "Foo").is_some());
}

#[test]
fn test_issue_resolve_call_backs() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, flatten: true, ..Default::default() };
    let parse_result = read_location("callbacks-issue/swagger.yaml", &options);
    assert!(parse_result.openapi.is_some());
    let spec = api(&parse_result);
    let callback = path(&spec, "/webhook").get_post().unwrap().get_callbacks().unwrap().get("WebhookVerificationEvent").unwrap();
    assert_eq!(callback.get_ref().as_deref(), Some("#/components/callbacks/WebhookVerificationEvent"));
}

#[test]
fn test_issue1190() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, flatten: true, ..Default::default() };
    let open_api = api(&read_location("issue1190/issue1190.yaml", &options));
    assert!(schema(&open_api, "SomeObj_lorem").is_some());
}

#[test]
fn test_issue1147() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, flatten: true, ..Default::default() };
    let parse_result = read_location("issue-1147/issue1147.yaml", &options);
    assert!(parse_result.openapi.is_some());
    let apispec = api(&parse_result);
    assert_eq!(prop(&schema(&apispec, "StringObject").unwrap(), "val").unwrap().get_ref().as_deref(), Some("#/components/schemas/String"));
}

#[test]
fn test_issue1148_flatten_dot() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { flatten: true, ..Default::default() };
    let parse_result = read_location("issue1148.yaml", &options);
    assert!(parse_result.openapi.is_some());
    let apispec = api(&parse_result);
    assert_eq!(prop(&schema(&apispec, "Some.User").unwrap(), "address").unwrap().get_ref().as_deref(), Some("#/components/schemas/Some.User_address"));
}

#[test]
fn test_issue1169() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, ..Default::default() };
    let parse_result = read_location("issue1169.yaml", &options);
    assert_eq!(messages(&parse_result).len(), 0);
    assert!(parse_result.openapi.is_some());
}

#[test]
fn test_issue1169no_split() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, ..Default::default() };
    let parse_result = read_location("issue1169-noSplit.yaml", &options);
    assert_eq!(messages(&parse_result).len(), 0);
    assert!(parse_result.openapi.is_some());
}

#[test]
fn test_issue339() {
    let _g = common::setup(common::V3_MODULE);
    let api = OpenAPIV3Parser::new().read("issue-339.yaml");
    assert!(api.is_some());
    let api = api.unwrap();
    let param = path(&api, "/store/order/{orderId}").get_get().unwrap().get_parameters().unwrap().get(0).unwrap();
    assert_eq!(param.kind(), ParameterKind::Path);
    let s = param.get_schema().unwrap();
    assert_eq!(s.get_minimum().unwrap().to_java_string(), "1");
    assert_eq!(s.get_maximum().unwrap().to_java_string(), "5");
}

/// Restores the process-wide `OpenAPIV3Parser.encoding` the test changes.
struct RestoreEncoding(String);

impl Drop for RestoreEncoding {
    fn drop(&mut self) {
        OpenAPIV3Parser::new().set_encoding(&self.0).unwrap();
    }
}

#[test]
fn test_issue1119() {
    let _g = common::setup(common::V3_MODULE);
    let _restore = RestoreEncoding(OpenAPIV3Parser::get_encoding());
    let parser = OpenAPIV3Parser::new();
    parser.set_encoding("ISO-8859-1").unwrap();
    let open_api = parser.read("src/test/resources/issue-1119.yaml");
    assert!(open_api.is_some());
    let open_api = open_api.unwrap();
    let param = path(&open_api, "/pets").get_get().unwrap().get_parameters().unwrap().get(0).unwrap();
    assert_eq!(param.get_description().as_deref(), Some("Cuántos artículos devolver al mismo tiempo (máximo 100)"));
}

#[test]
fn test_issue1108() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPIV3Parser::new().read("issue-1108.yaml");
    assert!(open_api.is_some());
    let open_api = open_api.unwrap();
    assert!(path(&open_api, "/pets").get_get().unwrap().get_parameters().unwrap().get(0).unwrap().get_allow_reserved().is_some());
}

#[test]
fn test_remote_parameter_issue1094() {
    let _g = common::setup(common::V3_MODULE);
    let result = OpenAPIV3Parser::new().read("issue-1094/swagger.yaml");
    assert!(result.is_some());
    assert!(schema(&result.unwrap(), "PlmnId").is_some());
}

fn mapschema_response(location: &str) -> Schema {
    let options = ParseOptions { resolve: true, ..Default::default() };
    let parse_result = read_location(location, &options);
    assert!(parse_result.openapi.is_some());
    let apispec = api(&parse_result);
    content_schema(&response(&path(&apispec, "/mapschema").get_get().unwrap(), "200"), "application/json")
}

#[test]
fn test_issue1071() {
    let _g = common::setup(common::V3_MODULE);
    let test = mapschema_response("issue-1071.yaml");
    assert_eq!(test.kind(), SchemaKind::Map);
}

#[test]
fn test_issue1071_true() {
    let _g = common::setup(common::V3_MODULE);
    let test = mapschema_response("issue-1071-true.yaml");
    assert_eq!(test.kind(), SchemaKind::Map);
    assert_eq!(test.get_additional_properties(), Some(AdditionalProperties::Bool(true)));
}

#[test]
fn test_issue1071_false() {
    let _g = common::setup(common::V3_MODULE);
    let test = mapschema_response("issue-1071-false.yaml");
    assert_eq!(test.kind(), SchemaKind::Object);
    assert_eq!(test.get_additional_properties(), Some(AdditionalProperties::Bool(false)));
}

#[test]
fn test_issue1039() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, ..Default::default() };
    let parse_result = read_location("issue_1039.yaml", &options);
    assert!(parse_result.openapi.is_some());
    let param = path(&api(&parse_result), "/pets").get_get().unwrap().get_parameters().unwrap().get(0).unwrap();
    assert_eq!(param.get_schema().unwrap().get_type().as_deref(), Some("array"));
}

#[test]
fn test_issue1015() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, resolve_combinators: true, ..Default::default() };
    let parse_result = read_location("issue-1015.json", &options);
    if let Some(m) = &parse_result.messages && !m.is_empty() {
        for message in m {
            println!("{message}");
        }
        panic!("Error while loading apispec!");
    }
    assert!(parse_result.openapi.is_some());
}

#[test]
fn test_issue_integer_default() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, ..Default::default() };
    let open_api = read_location("integerDefault.yaml", &options).openapi;
    assert!(open_api.is_some());
    let open_api = open_api.unwrap();
    let body = path(&open_api, "/fileUpload").get_post().unwrap().get_request_body().unwrap();
    let int_metadata = prop(&body.get_content().unwrap().get("multipart/form-data").unwrap().get_schema().unwrap(), "intMetadata").unwrap();
    assert_eq!(int_metadata.kind(), SchemaKind::Integer);
    // JDK-specific: `assertNotSame(format, "int32")` compares String identity with the interned literal that
    // `new IntegerSchema()` assigns; Rust strings have no identity. Java yields a null format here (SchemaTypeUtil
    // builds `new IntegerSchema().format(null)` for a blank format), which is what is checked instead.
    assert!(int_metadata.get_format().is_none());
    let get = path(&open_api, "/mockResponses/primitiveDoubleResponse").get_get().unwrap();
    assert!(content_schema(&response(&get, "200"), "application/json").get_format().is_none());
    let get = path(&open_api, "/issue-125").get_get().unwrap();
    assert!(content_schema(&response(&get, "200"), "*/*").get_format().is_none());
    let post = path(&open_api, "/primitiveBody/binary").get_post().unwrap();
    assert!(post.get_request_body().unwrap().get_content().unwrap().get("application/octet-stream").unwrap().get_schema().unwrap().get_format().is_none());
}

#[test]
fn test_issue983() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, ..Default::default() };
    let open_api = read_location("issue-983.yaml", &options).openapi;
    assert!(open_api.is_some());
    assert!(schema(&open_api.unwrap(), "InventoryId").is_some());
}

#[test]
fn test_issue913() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, ..Default::default() };
    let open_api = read_location("issue-913/BS/ApiSpecification.yaml", &options).openapi;
    assert!(open_api.is_some());
    let open_api = open_api.unwrap();
    assert!(schema(&open_api, "indicatorType").is_some());
    assert_eq!(schema(&open_api, "indicatorType").unwrap().get_properties().unwrap().len(), 1);
}

#[test]
fn test_issue901_2() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, ..Default::default() };
    let open_api = read_location("issue-901/spec2.yaml", &options).openapi;
    assert!(open_api.is_some());
    let open_api = open_api.unwrap();
    assert!(open_api.get_components().is_some());
    let array_schema = prop(&schema(&open_api, "Test.Definition").unwrap(), "stuff").unwrap();
    assert!(array_schema.is_array_schema());
    let internal_ref = array_schema.get_items().unwrap().get_ref();
    assert_eq!(internal_ref.as_deref(), Some("#/components/schemas/TEST.THING.OUT.Stuff"));
}

#[test]
fn test_issue901() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, ..Default::default() };
    let open_api = read_location("issue-901/spec.yaml", &options).openapi;
    assert!(open_api.is_some());
    let open_api = open_api.unwrap();
    let internal_ref = content_schema(&response(&path(&open_api, "/test").get_put().unwrap(), "200"), "application/json").get_ref();
    assert_eq!(internal_ref.as_deref(), Some("#/components/schemas/Test.Definition"));
    assert!(open_api.get_components().is_some());
}

/// `assertEquals(((LinkedHashMap<String, Object>) components.examples.testExample.value).get("test"), "value")`
fn assert_test_example(open_api: &OpenAPI) {
    let components = open_api.get_components();
    assert!(components.is_some());
    let examples = components.unwrap().get_examples();
    assert!(examples.is_some());
    let example = examples.unwrap().get("testExample");
    assert!(example.is_some());
    let Some(JavaObject::Map(map)) = example.unwrap().get_value() else { panic!("LinkedHashMap value expected") };
    assert!(matches!(map.get("test"), Some(JavaObject::String(s)) if &**s == "value"));
}

#[test]
fn test_issue853() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, flatten: true, ..Default::default() };
    let open_api = read_location("issue-837-853-1131/main.yaml", &options).openapi;
    assert!(open_api.is_some());
    let open_api = open_api.unwrap();

    let post = path(&open_api, "/guests").get_post();
    assert!(post.is_some());
    let content = response(&post.unwrap(), "201").get_content();
    assert!(content.is_some());
    let examples = content.unwrap().get("application/json").unwrap().get_examples().unwrap();
    assert_eq!(examples.len(), 1);
    assert_test_example(&open_api);
}

/// The three examples of `/events` (`plain`, `local`, `external`).
fn assert_events_examples(examples: &JMap<swagger_parser::models::Example>) {
    assert_eq!(examples.len(), 3);
    let Some(JavaObject::Node(node)) = examples.get("plain").unwrap().get_value() else { panic!("ObjectNode value expected") };
    assert!(node.is_object());
    assert_eq!(node.get("test").unwrap().as_text(), "plain");
    assert_eq!(examples.get("local").unwrap().get_ref().as_deref(), Some("#/components/examples/LocalRef"));
    assert_eq!(examples.get("external").unwrap().get_ref().as_deref(), Some("#/components/examples/ExternalRef"));
}

#[test]
fn test_issue837() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, ..Default::default() };
    let open_api = read_location("issue-837-853-1131/main.yaml", &options).openapi;
    assert!(open_api.is_some());
    let open_api = open_api.unwrap();
    let content = response(&path(&open_api, "/events").get_get().unwrap(), "200").get_content();
    assert!(content.is_some());
    assert_events_examples(&content.unwrap().get("application/json").unwrap().get_examples().unwrap());
}

#[test]
fn test_issue1131() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, ..Default::default() };
    let open_api = read_location("issue-837-853-1131/main.yaml", &options).openapi;
    assert!(open_api.is_some());
    let open_api = open_api.unwrap();
    let content = path(&open_api, "/events").get_get().unwrap().get_request_body().unwrap().get_content();
    assert!(content.is_some());
    assert_events_examples(&content.unwrap().get("application/json").unwrap().get_examples().unwrap());

    // Also cover the case from Issue 853
    let post = path(&open_api, "/guests").get_post();
    assert!(post.is_some());
    let content = post.unwrap().get_request_body().unwrap().get_content();
    assert!(content.is_some());
    let examples = content.unwrap().get("application/json").unwrap().get_examples().unwrap();
    assert_eq!(examples.len(), 1);
    assert_test_example(&open_api);
}

#[test]
fn test_issue834() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, resolve_responses: true, ..Default::default() };
    let result = read_location("issue-834/index.yaml", &options);
    assert!(result.openapi.is_some());
    let open_api = api(&result);
    for (p, code) in [("/foo", "200"), ("/foo", "300"), ("/bar", "200")] {
        let content = response(&path(&open_api, p).get_get().unwrap(), code).get_content();
        assert!(content.is_some());
        let schema_ref = content.unwrap().get("application/json").unwrap().get_schema().unwrap().get_ref();
        assert_eq!(schema_ref.as_deref(), Some("#/components/schemas/schema"));
    }
}

#[test]
fn test_issue811_ref_schema_to_ref_schema() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, ..Default::default() };
    let open_api = read_location("oapi-reference-test2/index.yaml", &options).openapi;
    assert!(open_api.is_some());
    let open_api = open_api.unwrap();
    let s = content_schema(&response(&path(&open_api, "/").get_get().unwrap(), "200"), "application/json");
    assert_eq!(s.get_ref().as_deref(), Some("#/components/schemas/schema-with-reference"));
}

#[test]
fn test_issue811() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, resolve_responses: true, ..Default::default() };
    let open_api = read_location("oapi-reference-test/index.yaml", &options).openapi;
    assert!(open_api.is_some());
    let open_api = open_api.unwrap();
    let s = content_schema(&response(&path(&open_api, "/").get_get().unwrap(), "200"), "application/json");
    assert_eq!(s.get_ref().as_deref(), Some("#/components/schemas/schema-with-reference"));
}

#[test]
fn test_issue719() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = read_location("extensions-responses.yaml", &ParseOptions::default()).openapi;
    assert!(open_api.is_some());
    let open_api = open_api.unwrap();
    assert!(open_api.get_paths().unwrap().get_extensions().is_some());
    assert!(path(&open_api, "/something").get_get().unwrap().get_responses().unwrap().get_extensions().is_some());
}

#[test]
fn issue682() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, ..Default::default() };
    let result = read_location("src/test/resources/sample/SwaggerPetstore.yaml", &options);
    assert!(result.openapi.is_some());
    assert!(messages(&result).is_empty());
    assert!(path(&api(&result), "/pets").get_get().is_some());
}

#[test]
fn issue941() {
    let _g = common::setup(common::V3_MODULE);
    let result = OpenAPIV3Parser::new().read("src/test/resources/sample/SwaggerPetstore.yaml");
    assert!(result.is_some());
    assert_eq!(result.unwrap().get_info().unwrap().get_description().as_deref(), Some("Documentation de l'API élaboré par nos soins"));
}

#[test]
fn issue_relative_refs2() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, ..Default::default() };
    let result = read_location("src/test/resources/relative-upper-directory/swagger.yaml", &options);
    assert!(result.openapi.is_some());
    let open_api = api(&result);
    assert!(path(&open_api, "/api/Address").get_get().is_some());
    assert_eq!(schemas(&open_api).len(), 1);
    assert!(schema(&open_api, "AddressEx").is_some());
}

#[test]
fn test_pattern() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = read_location("testPattern.yaml", &ParseOptions::default()).openapi.unwrap();
    let s = schema(&open_api, "SomeObj").unwrap();
    assert_eq!(s.get_pattern().as_deref(), Some("^[A-Z]+$"));
}

#[test]
fn test_resolve_empty() {
    let _g = common::setup(common::V3_MODULE);
    let path_file = common::read_file("src/test/resources/empty-oas.yaml");
    let options = ParseOptions { resolve_fully: true, ..Default::default() };
    let result = OpenAPIV3Parser::new().read_contents(&path_file, Some(&auths()), Some(&options));
    assert!(result.openapi.is_some());
}

#[test]
fn test_remote_path_item_issue1103() {
    let _g = common::setup(common::V3_MODULE);
    let result = OpenAPIV3Parser::new().read("issue-1103/remote-pathItem-swagger.yaml");
    assert!(result.is_some());
    let result = result.unwrap();
    assert!(result.get_paths().unwrap().get("/Translation/{lang}").is_some());
    let param = path(&result, "/Translation/{lang}").get_put().unwrap().get_parameters().unwrap().get(0).unwrap();
    assert_eq!(param.get_name().as_deref(), Some("lang"));
}

#[test]
fn test_remote_parameter_issue1103() {
    let _g = common::setup(common::V3_MODULE);
    let result = OpenAPIV3Parser::new().read("issue-1103/remote-parameter-swagger.yaml");
    assert!(result.is_some());
    let param = path(&result.unwrap(), "/Translation/{lang}").get_put().unwrap().get_parameters().unwrap().get(0).unwrap();
    assert_eq!(param.get_name().as_deref(), Some("lang"));
}

#[test]
fn test_issue1105() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPIV3Parser::new().read("issue-1105/swagger-api.yaml");
    assert!(open_api.is_some());
    assert!(schema(&open_api.unwrap(), "ErrorCodeDescription").is_some());
}

#[test]
fn test_ref_additional_properties() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPIV3Parser::new().read("src/test/resources/relative/additionalProperties.yaml");
    assert!(open_api.is_some());
    let open_api = open_api.unwrap();
    assert_eq!(schemas(&open_api).len(), 3);
    assert!(schema(&open_api, "link-object").is_some());
    assert!(schema(&open_api, "rel-data").is_some());
    assert!(schema(&open_api, "result").is_some());
}

#[test]
fn test_ref_and_inline_all_of() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, resolve_fully: true, ..Default::default() };
    let open_api = OpenAPIV3Parser::new().read_with(Some("src/test/resources/allOfAndRef.yaml"), Some(&auths()), Some(&options));
    assert!(open_api.is_some());
    let open_api = open_api.unwrap();
    assert_eq!(schemas(&open_api).len(), 2);
    assert!(schema(&open_api, "UserEx").is_some());
    assert!(schema(&open_api, "User").is_some());
    let s = content_schema(&response(&path(&open_api, "/refToAllOf").get_get().unwrap(), "200"), "application/json");
    assert_eq!(s.get_properties().unwrap().len(), 2);
}

#[test]
fn test_composed_ref_resolving_issue628() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, ..Default::default() };
    let open_api = OpenAPIV3Parser::new().read_with(Some("src/test/resources/composedSchemaRef.yaml"), Some(&auths()), Some(&options));
    assert!(open_api.is_some());
    let open_api = open_api.unwrap();
    assert_eq!(schemas(&open_api).len(), 5);
    for name in ["Cat", "Dog", "Pet", "Lion", "Bear"] {
        assert!(schema(&open_api, name).is_some(), "{name}");
    }
}

#[test]
fn test_composed_schema_adjacent() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, ..Default::default() };
    let open_api = OpenAPIV3Parser::new().read_with(Some("src/test/resources/composedSchemaRef.yaml"), Some(&auths()), Some(&options));
    assert!(open_api.is_some());
    let open_api = open_api.unwrap();
    assert_eq!(schemas(&open_api).len(), 5);
    let s = content_schema(&response(&path(&open_api, "/path").get_get().unwrap(), "200"), "application/json");
    assert!(s.is_composed_schema());
    assert_eq!(s.get_one_of().unwrap().len(), 2);
    assert_eq!(s.get_all_of().unwrap().len(), 1);
}

#[test]
fn test_one_of_external_ref_conflict_name() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPIV3Parser::new().read("./oneof_name_conflict/oneOf-external-ref-name-conflict.yaml");
    assert!(open_api.is_some());
    let pet = schema(&open_api.unwrap(), "Pet");
    assert!(pet.is_some());
    let mapping = pet.unwrap().get_discriminator().unwrap().get_mapping().unwrap();
    assert!(mapping.contains_key("Cat"));
    assert_eq!(mapping.get("Cat").as_deref(), Some("#/components/schemas/Cat_1"));
}

#[test]
fn int64_example_without_overflow() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPIV3Parser::new().read("src/test/resources/int64example.yaml").unwrap();
    let s = content_schema(&response(&path(&open_api, "/foo").get_get().unwrap(), "200"), "application/json");
    let date = prop(&s, "date").unwrap();
    assert_eq!(date.kind(), SchemaKind::Integer);
    assert_eq!(java_string(date.get_example()).as_deref(), Some("1516042231144"));
}

#[test]
fn test_ref_paths() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = "openapi: '3.0.0'\n\
info:\n\
\x20 version: 0.0.0\n\
\x20 title: Path item $ref test\n\
paths:\n\
\x20 /foo:\n\
\x20   get:\n\
\x20     responses:\n\
\x20       200:\n\
\x20         description: OK\n\
\x20 /foo2:\n\
\x20   $ref: \"#/paths/~1foo\"";
    let open_api = read_contents(yaml, None).openapi.unwrap();
    let paths = open_api.get_paths().unwrap();
    assert_eq!(paths.get("foo"), paths.get("foo2"));
}

#[test]
fn test_model_parameters() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = "openapi: '2.0'\n\
info:\n\
\x20 version: \"0.0.0\"\n\
\x20 title: test\n\
paths:\n\
\x20 /foo:\n\
\x20   get:\n\
\x20     parameters:\n\
\x20     - name: amount\n\
\x20       in: body\n\
\x20       schema:\n\
\x20         type: integer\n\
\x20         format: int64\n\
\x20         description: amount of money\n\
\x20         default: 1000\n\
\x20         maximum: 100000\n\
\x20     responses:\n\
\x20       200:\n\
\x20         description: ok";
    // No assertions: the call must not throw (`.getOpenAPI()` on the returned result)
    let _open_api = read_contents(yaml, None).openapi;
}

#[test]
fn test_parse_shared_path_parameters() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = "openapi: '3.0.0'\n\
info:\n\
\x20 version: \"0.0.0\"\n\
\x20 title: test\n\
paths:\n\
\x20 /persons/{id}:\n\
\x20   parameters:\n\
\x20     - in: path\n\
\x20       name: id\n\
\x20       type: string\n\
\x20       required: true\n\
\x20       description: \"no\"\n\
\x20   get:\n\
\x20     parameters:\n\
\x20       - name: id\n\
\x20         in: path\n\
\x20         required: true\n\
\x20         type: string\n\
\x20         description: \"yes\"\n\
\x20       - name: name\n\
\x20         in: query\n\
\x20         type: string\n\
\x20     responses:\n\
\x20       200:\n\
\x20         description: ok\n";
    let open_api = read_contents(yaml, None).openapi.unwrap();
    let parameters = path(&open_api, "/persons/{id}").get_get().unwrap().get_parameters().unwrap();
    assert_eq!(parameters.len(), 2);
    let id = parameters.get(0).unwrap();
    assert_eq!(id.get_description().as_deref(), Some("yes"));
}

#[test]
fn test_parse_ref_path_parameters() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = "openAPI: '2.0'\n\
info:\n\
\x20 title: test\n\
\x20 version: '0.0.0'\n\
parameters:\n\
\x20 report-id:\n\
\x20   name: id\n\
\x20   in: path\n\
\x20   type: string\n\
\x20   required: true\n\
paths:\n\
\x20 /reports/{id}:\n\
\x20   parameters:\n\
\x20       - $ref: \"#/parameters/report-id\"\n\
\x20   put:\n\
\x20     parameters:\n\
\x20       - name: id\n\
\x20         in: body\n\
\x20         required: true\n\
\x20         schema:\n\
\x20           $ref: \"#/components/schemas/report\"\n\
\x20     responses:\n\
\x20       200:\n\
\x20         description: ok\n\
definitions:\n\
\x20 report:\n\
\x20   type: object\n\
\x20   properties:\n\
\x20     id:\n\
\x20       type: string\n\
\x20     name:\n\
\x20       type: string\n\
\x20   required:\n\
\x20   - id\n\
\x20   - name\n";
    // No assertions: the call must not throw (`.getOpenAPI()` on the returned result)
    let _open_api = read_contents(yaml, None).openapi;
}

#[test]
fn test_unique_parameters() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = "openapi: 3.0.0\n\
servers: []\n\
info:\n\
\x20 title: test\n\
\x20 version: 0.0.0\n\
paths:\n\
\x20 '/foos/{id}':\n\
\x20   parameters:\n\
\x20     - $ref: \"#/components/parameters/foo-id\"\n\
\x20   get:\n\
\x20     responses:\n\
\x20       '200':\n\
\x20         description: OK\n\
\x20         content:\n\
\x20           '*/*':\n\
\x20             schema:\n\
\x20               $ref: \"#/components/schemas/foo\"\n\
\x20   put:\n\
\x20     responses:\n\
\x20       '200':\n\
\x20         description: OK\n\
\x20         content:\n\
\x20           '*/*':\n\
\x20             schema:\n\
\x20               $ref: \"#/components/schemas/foo\"\n\
\x20     requestBody:\n\
\x20       content:\n\
\x20         application/json:\n\
\x20           schema:\n\
\x20             $ref: \"#/components/schemas/foo\"\n\
\x20       required: true\n\
components:\n\
\x20 parameters:\n\
\x20   foo-id:\n\
\x20     name: id\n\
\x20     in: path\n\
\x20     required: true\n\
\x20     schema:\n\
\x20       type: string\n\
\x20 schemas:\n\
\x20   foo:\n\
\x20     type: object\n\
\x20     properties:\n\
\x20       id:\n\
\x20         type: string\n\
\x20     required:\n\
\x20       - id";
    let options = ParseOptions { resolve: true, ..Default::default() };
    let open_api = read_contents(yaml, Some(&options)).openapi.unwrap();
    let put = path(&open_api, "/foos/{id}").get_put().unwrap();
    let parameters = put.get_parameters().unwrap();
    assert!(put.get_request_body().is_some());
    assert!(parameters.len() == 1);
}

/// `expectedPropertiesInModel`
fn expected_properties_in_model(model: &Schema, expected: &[&str]) {
    let props = model.get_properties().unwrap();
    assert_eq!(props.len(), expected.len());
    for p in expected {
        assert!(props.contains_key(p), "{p}");
    }
}

/// `assertResponse`
fn assert_response(open_api: &OpenAPI, op: &Operation, code: &str, media_type: &str, expected_description: &str, expected_schema_ref: &str) {
    let r = response(op, code);
    let s = content_schema(&r, media_type);
    assert_eq!(r.get_description().as_deref(), Some(expected_description));
    assert_eq!(s.kind(), SchemaKind::Schema);
    assert_eq!(s.get_ref().as_deref(), Some(expected_schema_ref));
    let r = s.get_ref().unwrap();
    assert!(schemas(open_api).contains_key(&r[r.rfind('/').unwrap() + 1..]));
}

/// `assertParamDetails`
fn assert_param_details(parameters: &swagger_parser::java::JList<swagger_parser::models::Parameter>, index: usize, expected_type: ParameterKind, expected_name: &str, expected_in: &str) {
    let param = parameters.get(index).unwrap();
    assert_eq!(param.kind(), expected_type);
    assert_eq!(param.get_name().as_deref(), Some(expected_name));
    assert_eq!(param.get_in().as_deref(), Some(expected_in));
}

/// `doRelativeFileTest`
fn do_relative_file_test(location: &str) -> OpenAPI {
    let options = ParseOptions { resolve: true, resolve_responses: true, ..Default::default() };
    let read_result = read_location(location, &options);
    if let Some(m) = &read_result.messages && !m.is_empty() {
        println!("{}", common::java_list(m));
    }
    let open_api = api(&read_result);
    // `path.getClass() == PathItem.class`: the model has no PathItem subclasses; the lookup must succeed
    let path = open_api.get_paths().unwrap().get("/health").unwrap();

    assert!(path.get_parameters().is_none());

    let operation = path.get_get().unwrap();
    let operation_params = operation.get_parameters().unwrap();
    assert_param_details(&operation_params, 0, ParameterKind::Query, "param1", "query");
    assert_param_details(&operation_params, 1, ParameterKind::Header, "param2", "header");
    assert_param_details(&operation_params, 2, ParameterKind::Path, "param3", "path");
    assert_param_details(&operation_params, 3, ParameterKind::Header, "param4", "header");

    assert_response(&open_api, &operation, "200", "application/json", "Health information from the server", "#/components/schemas/health");
    assert_response(&open_api, &operation, "400", "*/*", "Your request was not valid", "#/components/schemas/error");
    assert_response(&open_api, &operation, "500", "*/*", "An unexpected error occur during processing", "#/components/schemas/error");

    let definitions = schemas(&open_api);
    let ref_in_definitions = definitions.get("refInDefinitions").unwrap();
    assert_eq!(ref_in_definitions.get_description().as_deref(), Some("The example model"));
    expected_properties_in_model(&ref_in_definitions, &["foo", "bar"]);

    let array_model = definitions.get("arrayModel").unwrap();
    assert!(array_model.is_array_schema());
    assert_eq!(array_model.get_items().unwrap().get_ref().as_deref(), Some("#/components/schemas/foo"));

    let foo_model = definitions.get("foo").unwrap();
    assert_eq!(foo_model.get_description().as_deref(), Some("Just another model"));
    expected_properties_in_model(&foo_model, &["hello", "world"]);

    let composed_cat = definitions.get("composedCat").unwrap();
    assert!(composed_cat.is_composed_schema());
    let child = composed_cat.get_all_of().unwrap().get(2).unwrap();
    expected_properties_in_model(&child, &["huntingSkill", "prop2", "reflexes", "reflexMap"]);
    let reflexes = prop(&child, "reflexes").unwrap();
    assert!(reflexes.is_array_schema());
    let reflex_items = reflexes.get_items().unwrap();
    assert_eq!(reflex_items.get_ref().as_deref(), Some("#/components/schemas/reflex"));
    let r = reflex_items.get_ref().unwrap();
    assert!(definitions.contains_key(&r[r.rfind('/').unwrap() + 1..]));

    let reflex_map = prop(&child, "reflexMap").unwrap();
    let Some(AdditionalProperties::Schema(reflex_map_additional_properties)) = reflex_map.get_additional_properties() else { panic!("Schema additionalProperties expected") };
    assert_eq!(reflex_map_additional_properties.get_ref().as_deref(), Some("#/components/schemas/reflex"));

    assert_eq!(composed_cat.get_all_of().unwrap().len(), 3);
    assert_eq!(composed_cat.get_all_of().unwrap().get(0).unwrap().get_ref().as_deref(), Some("#/components/schemas/pet"));
    assert_eq!(composed_cat.get_all_of().unwrap().get(1).unwrap().get_ref().as_deref(), Some("#/components/schemas/foo_1"));

    open_api
}

#[test]
fn test_load_relative_file_tree_json() {
    let _g = common::setup(common::V3_MODULE);
    let _open_api = do_relative_file_test("src/test/resources/relative-file-references/json/parent.json");
}

#[test]
fn test_load_external_nested_definitions() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPIV3Parser::new().read("src/test/resources/nested-references/b.yaml").unwrap();
    let definitions = schemas(&open_api);
    assert!(definitions.contains_key("x"));
    assert!(definitions.contains_key("y"));
    assert!(definitions.contains_key("z"));
    assert_eq!(definitions.get("i").unwrap().get_ref().as_deref(), Some("#/components/schemas/k"));
    assert_eq!(definitions.get("k").unwrap().get_title().as_deref(), Some("k-definition"));
}

fn assert_nested_greeting(location: &str) {
    let open_api = OpenAPIV3Parser::new().read(location).unwrap();
    let definitions = schemas(&open_api);
    assert!(definitions.contains_key("GreetingResponse"));
    assert!(definitions.contains_key("GreetingMessage"));
    assert!(definitions.contains_key("greeting-message"));

    let greeting_message_ref = prop(&definitions.get("GreetingResponse").unwrap(), "GreetingMessage").unwrap().get_ref();
    assert_eq!(greeting_message_ref.as_deref(), Some("#/components/schemas/GreetingMessage"));
    assert_eq!(definitions.get("GreetingMessage").unwrap().get_ref().as_deref(), Some("#/components/schemas/greeting-message"));

    let description = prop(&definitions.get("greeting-message").unwrap(), "Text").unwrap().get_description();
    assert_eq!(description.as_deref(), Some("Text message"));
}

#[test]
fn test_load_external_nested_definitions_with_reference_within_subfolder() {
    let _g = common::setup(common::V3_MODULE);
    assert_nested_greeting("src/test/resources/nested-references-2/main.yaml");
}

#[test]
fn test_load_external_nested_definitions_with_reference_within_same_folder() {
    let _g = common::setup(common::V3_MODULE);
    assert_nested_greeting("src/test/resources/nested-references-3/main.yaml");
}

#[test]
fn test_load_external_nested_definitions_with_reference_on_different_folder_levels() {
    let _g = common::setup(common::V3_MODULE);
    assert_nested_greeting("src/test/resources/nested-references-4/main.yaml");
}

#[test]
fn test_petstore() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, ..Default::default() };
    let result = read_location("src/test/resources/petstore.yaml", &options);
    assert_eq!(messages(&result).len(), 2);

    let open_api = api(&result);
    let definitions = schemas(&open_api);
    let key_set: std::collections::HashSet<String> = definitions.keys().into_iter().collect();
    let expected: std::collections::HashSet<String> = ["User", "Category", "Pet", "Tag", "Order", "PetArray"].map(String::from).into();
    assert_eq!(key_set, expected);

    let pet_model = definitions.get("Pet").unwrap();
    let pet_props: std::collections::HashSet<String> = pet_model.get_properties().unwrap().keys().into_iter().collect();
    let expected_pet_props: std::collections::HashSet<String> = ["id", "category", "name", "photoUrls", "tags", "status"].map(String::from).into();
    assert_eq!(pet_props, expected_pet_props);

    let pet_array_model = definitions.get("PetArray").unwrap();
    assert!(pet_array_model.is_array_schema());
    assert_eq!(pet_array_model.get_type().as_deref(), Some("array"));
    let ref_prop = pet_array_model.get_items().unwrap();
    assert_eq!(ref_prop.get_ref().as_deref(), Some("#/components/schemas/Pet"));
    assert!(pet_array_model.get_properties().is_none());
}
