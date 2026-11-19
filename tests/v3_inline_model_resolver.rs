//! Port of `io.swagger.v3.parser.util.InlineModelResolverTest`.

mod common;

use jackson::{BigDecimal, JavaObject};
use swagger_parser::core::ParseOptions;
use swagger_parser::java::JList;
use swagger_parser::models::*;
use swagger_parser::parser::inline_model_resolver::InlineModelResolver;
use swagger_parser::parser::v3_parser::OpenAPIV3Parser;

/// `new InlineModelResolver()`
fn resolver() -> InlineModelResolver {
    InlineModelResolver::new(false, false, false)
}

/// `new InlineModelResolver(flattenComposedSchemas, camelCaseFlattenNaming)`
fn resolver2(flatten_composed_schemas: bool, camel_case_flatten_naming: bool) -> InlineModelResolver {
    InlineModelResolver::new(flatten_composed_schemas, camel_case_flatten_naming, false)
}

fn schemas(open_api: &OpenAPI) -> swagger_parser::java::JMap<Schema> {
    open_api.get_components().unwrap().get_schemas().unwrap()
}

fn schema_named(open_api: &OpenAPI, name: &str) -> Option<Schema> {
    schemas(open_api).get(name)
}

fn prop(s: &Schema, name: &str) -> Option<Schema> {
    s.get_properties().unwrap().get(name)
}

fn ext(s: &Schema, name: &str) -> Option<JavaObject> {
    s.get_extensions().unwrap().get(name)
}

fn content_schema(c: &Option<Content>, media: &str) -> Option<Schema> {
    c.as_ref().unwrap().get(media).unwrap().get_schema()
}

fn read_flatten(location: &str, resolve: bool) -> Option<OpenAPI> {
    let mut options = ParseOptions::new();
    if resolve {
        options.resolve = true;
    }
    options.flatten = true;
    OpenAPIV3Parser::new().read_with(Some(location), None, Some(&options))
}

#[test]
fn test_issue_unexpected_null_values() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = read_flatten("unexpectedNullValues.yaml", true).expect("openAPI");
    let dataset = schema_named(&open_api, "DatasetDetail_schema").unwrap();
    assert!(dataset.get_example().is_none());
    assert!(!dataset.get_example_set_flag());
    // example set to null explicitly in the definition
    let verify = schema_named(&open_api, "verify_datasets").unwrap();
    assert!(verify.get_example().is_none());
    assert!(verify.get_example_set_flag());
}

#[test]
fn test_issue1018() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = read_flatten("flatten.json", false).expect("openAPI");
    assert!(schema_named(&open_api, "ReturnInformation_manufacturer_signin_credentials").unwrap().get_required().is_some());
}

#[test]
fn test_issue705() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = read_flatten("issue-705.yaml", false).expect("openAPI");
    assert!(schema_named(&open_api, "inline_response_200").unwrap().get_type().is_some());
}

/// The address object schema shared by several tests.
fn address_object_schema(title: Option<&str>) -> Schema {
    let object_schema = Schema::object_schema();
    if let Some(t) = title {
        object_schema.set_title(t);
    }
    object_schema.set_default("default");
    object_schema.set_read_only(false);
    object_schema.set_description("description");
    object_schema.set_name("name");
    object_schema.add_properties("street", Schema::string_schema());
    object_schema.add_properties("city", Schema::string_schema());
    object_schema
}

fn user_schema(address: Schema) -> Schema {
    let schema = Schema::new();
    schema.set_name("user");
    schema.set_description("a common user");
    schema.set_required(JList::from_vec(vec!["address".to_string()]));
    schema.add_properties("name", Schema::string_schema());
    schema.add_properties("address", address);
    schema
}

#[test]
fn resolve_inline_model_test_without_title() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPI::new();
    open_api.set_components(Components::new());
    open_api.get_components().unwrap().add_schemas("User", user_schema(address_object_schema(None)));

    resolver().flatten(&open_api).unwrap();

    let user = schema_named(&open_api, "User").unwrap();
    let address = prop(&user, "address").unwrap();
    assert!(address.get_ref().is_some());
    let user_address = schema_named(&open_api, "User_address").unwrap();
    assert!(prop(&user_address, "city").is_some());
    assert!(prop(&user_address, "street").is_some());
}

#[test]
fn resolve_inline_model_test_with_title() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPI::new();
    open_api.set_components(Components::new());
    open_api.get_components().unwrap().add_schemas("User", user_schema(address_object_schema(Some("UserAddressTitle"))));

    resolver().flatten(&open_api).unwrap();

    let user = schema_named(&open_api, "User").unwrap();
    let address = prop(&user, "address").unwrap();
    assert!(address.get_ref().is_some());
    let t = schema_named(&open_api, "UserAddressTitle").unwrap();
    assert!(prop(&t, "city").is_some());
    assert!(prop(&t, "street").is_some());
}

#[test]
fn resolve_inline_model2_equal_inner_models() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPI::new();
    open_api.set_components(Components::new());
    open_api.get_components().unwrap().add_schemas("User", user_schema(address_object_schema(Some("UserAddressTitle"))));

    let address_schema = address_object_schema(Some("UserAddressTitle"));
    let another_schema = Schema::new();
    another_schema.set_name("user");
    another_schema.set_description("a common user");
    another_schema.set_required(JList::from_vec(vec!["address".to_string()]));
    another_schema.add_properties("name", Schema::string_schema());
    another_schema.add_properties("lastName", Schema::string_schema());
    another_schema.add_properties("address", address_schema);
    open_api.get_components().unwrap().add_schemas("AnotherUser", another_schema);

    resolver().flatten(&open_api).unwrap();

    let user = schema_named(&open_api, "User").unwrap();
    assert!(prop(&user, "address").unwrap().get_ref().is_some());
    let address = schema_named(&open_api, "UserAddressTitle").unwrap();
    assert!(prop(&address, "city").is_some());
    assert!(prop(&address, "street").is_some());
    assert!(schema_named(&open_api, "UserAddressTitle_0").is_none());
}

#[test]
fn resolve_inline_model2_different_inner_models_with_same_title() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPI::new();
    open_api.set_components(Components::new());
    open_api.get_components().unwrap().add_schemas("User", user_schema(address_object_schema(Some("UserAddressTitle"))));

    let address_schema = address_object_schema(Some("UserAddressTitle"));
    address_schema.add_properties("apartment", Schema::string_schema());
    let another_schema = Schema::new();
    another_schema.set_name("AnotherUser");
    another_schema.set_description("a common user");
    another_schema.set_required(JList::from_vec(vec!["address".to_string()]));
    another_schema.add_properties("name", Schema::string_schema());
    another_schema.add_properties("lastName", Schema::string_schema());
    another_schema.add_properties("address", address_schema);
    open_api.get_components().unwrap().add_schemas("AnotherUser", another_schema);

    resolver().flatten(&open_api).unwrap();

    let user = schema_named(&open_api, "User").unwrap();
    assert!(prop(&user, "address").unwrap().get_ref().is_some());
    let address = schema_named(&open_api, "UserAddressTitle").unwrap();
    assert!(prop(&address, "city").is_some());
    assert!(prop(&address, "street").is_some());
    let duplicate = schema_named(&open_api, "UserAddressTitle_1").unwrap();
    assert!(prop(&duplicate, "city").is_some());
    assert!(prop(&duplicate, "street").is_some());
    assert!(prop(&duplicate, "apartment").is_some());
}

#[test]
fn test_inline_response_model() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPI::new();

    let object_schema1 = Schema::object_schema();
    object_schema1.add_properties("name", Schema::string_schema());
    object_schema1.add_extension("x-ext", JavaObject::string("ext-prop"));
    let media_type1 = MediaType::new();
    media_type1.set_schema(object_schema1);
    let content1 = Content::new();
    content1.add_media_type("*/*", media_type1);
    let response1 = ApiResponse::new();
    response1.set_description("it works!");
    response1.set_content(content1);
    let responses1 = ApiResponses::new();
    responses1.add_api_response("200", response1);
    let operation1 = Operation::new();
    operation1.set_responses(responses1);
    let path_item1 = PathItem::new();
    path_item1.set_get(operation1);
    open_api.path("/foo/bar", path_item1);

    let object_schema2 = Schema::object_schema();
    object_schema2.add_properties("name", Schema::string_schema());
    object_schema2.add_extension("x-ext", JavaObject::string("ext-prop"));
    let media_type2 = MediaType::new();
    media_type2.set_schema(object_schema2);
    let content2 = Content::new();
    content2.add_media_type("*/*", media_type2);
    let response2 = ApiResponse::new();
    response2.set_description("it works!");
    response2.add_extension("x-foo", JavaObject::string("bar"));
    response2.set_content(content2);
    let responses2 = ApiResponses::new();
    responses2.add_api_response("200", response2);
    let operation2 = Operation::new();
    operation2.set_responses(responses2);
    let path_item2 = PathItem::new();
    path_item2.set_get(operation2);
    open_api.path("/foo/baz", path_item2);

    resolver().flatten(&open_api).unwrap();

    let responses = open_api.get_paths().unwrap().get("/foo/bar").unwrap().get_get().unwrap().get_responses().unwrap();
    let response = responses.get("200").unwrap();
    let schema = content_schema(&response.get_content(), "*/*").unwrap();
    assert!(schema.get_ref().is_some());
    assert_eq!(1, schema.get_extensions().unwrap().len());
    assert_eq!(Some(JavaObject::string("ext-prop")), ext(&schema, "x-ext"));

    let model = schema_named(&open_api, "inline_response_200").unwrap();
    assert!(model.get_properties().unwrap().len() == 1);
    assert!(prop(&model, "name").is_some());
    assert!(prop(&model, "name").unwrap().is_kind(SchemaKind::String));
}

#[test]
fn test_inline_response_model_with_title() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPI::new();
    let response_title = "GetBarResponse";

    let object_schema1 = Schema::object_schema();
    object_schema1.set_title(response_title);
    object_schema1.add_properties("name", Schema::string_schema());
    let media_type1 = MediaType::new();
    media_type1.set_schema(object_schema1);
    let content1 = Content::new();
    content1.add_media_type("*/*", media_type1);
    let response1 = ApiResponse::new();
    response1.set_description("it works!");
    response1.set_content(content1);
    let responses1 = ApiResponses::new();
    responses1.add_api_response("200", response1);
    let operation1 = Operation::new();
    operation1.set_responses(responses1);
    let path_item1 = PathItem::new();
    path_item1.set_get(operation1);
    open_api.path("/foo/bar", path_item1);

    let object_schema2 = Schema::object_schema();
    object_schema2.add_properties("name", Schema::string_schema());
    object_schema2.add_extension("x-foo", JavaObject::string("bar"));
    let media_type2 = MediaType::new();
    media_type2.set_schema(object_schema2);
    let content2 = Content::new();
    content2.add_media_type("*/*", media_type2);
    let response2 = ApiResponse::new();
    response2.set_description("it works!");
    response2.set_content(content2);
    let responses2 = ApiResponses::new();
    responses2.add_api_response("200", response2);
    let operation2 = Operation::new();
    operation2.set_responses(responses2);
    let path_item2 = PathItem::new();
    path_item2.set_get(operation2);
    open_api.path("/foo/baz", path_item2);

    resolver().flatten(&open_api).unwrap();

    let responses = open_api.get_paths().unwrap().get("/foo/bar").unwrap().get_get().unwrap().get_responses().unwrap();
    let response = responses.get("200").unwrap();
    assert!(content_schema(&response.get_content(), "*/*").unwrap().get_ref().is_some());

    let model = schema_named(&open_api, response_title).unwrap();
    assert!(model.get_properties().unwrap().len() == 1);
    assert!(prop(&model, "name").is_some());
    assert!(prop(&model, "name").unwrap().is_kind(SchemaKind::String));
}

fn open_api_with_inline_response_schema(title: &str) -> OpenAPI {
    let inline_schema = Schema::object_schema().title(title).add_property("name", Schema::string_schema());
    let response = ApiResponse::new()
        .description("OK")
        .content(Content::new().add_media_type("application/json", MediaType::new().schema(inline_schema)));
    let operation = Operation::new().responses(ApiResponses::new().add_api_response("200", response));
    OpenAPI::new().path("/admin.user", PathItem::new().get(operation))
}

fn assert_flattened_response_schema(open_api: &OpenAPI, expected_name: &str, expected_ref: &str) {
    assert!(open_api.get_components().is_some());
    assert!(open_api.get_components().unwrap().get_schemas().is_some());
    assert!(schemas(open_api).contains_key(expected_name));
    let resolved = open_api.get_paths().unwrap().get("/admin.user").unwrap().get_get().unwrap().get_responses().unwrap().get("200").unwrap();
    let resolved_schema = content_schema(&resolved.get_content(), "application/json").unwrap();
    assert_eq!(Some(expected_ref.to_string()), resolved_schema.get_ref());
    assert!(expected_ref.starts_with("#/components/schemas/"));
    assert!(schemas(open_api).contains_key(&expected_ref["#/components/schemas/".len()..]));
}

#[test]
fn test_flatten_schema_title_with_dot() {
    let _g = common::setup(common::V3_MODULE);
    let title = "admin.user";
    let open_api = open_api_with_inline_response_schema(title);
    resolver().flatten(&open_api).unwrap();
    // A dot is valid in a component key and must not make the generated $ref path-like or external.
    assert_flattened_response_schema(&open_api, title, "#/components/schemas/admin.user");
}

#[test]
fn test_flatten_schema_title_with_simple_name() {
    let _g = common::setup(common::V3_MODULE);
    let title = "User";
    let open_api = open_api_with_inline_response_schema(title);
    resolver().flatten(&open_api).unwrap();
    assert_flattened_response_schema(&open_api, title, "#/components/schemas/User");
}

fn dotted_object_schema(title: &str) -> Schema {
    Schema::object_schema().title(title).add_property("name", Schema::string_schema())
}

fn request_body(schema: Schema) -> RequestBody {
    RequestBody::new().content(Content::new().add_media_type("application/json", MediaType::new().schema(schema)))
}

fn response(schema: Schema) -> ApiResponse {
    ApiResponse::new().description("OK").content(Content::new().add_media_type("application/json", MediaType::new().schema(schema)))
}

fn open_api_with_response_schema(schema: Schema) -> OpenAPI {
    OpenAPI::new().path("/response", PathItem::new().get(Operation::new().responses(ApiResponses::new().add_api_response("200", response(schema)))))
}

fn open_api_with_parameter(parameter: Parameter) -> OpenAPI {
    OpenAPI::new().path("/parameter", PathItem::new().get(Operation::new().add_parameters_item(parameter)))
}

fn assert_local_schema_ref(open_api: &OpenAPI, model_name: &str, reference: &Schema) {
    assert!(open_api.get_components().is_some());
    assert!(open_api.get_components().unwrap().get_schemas().is_some());
    assert!(schemas(open_api).contains_key(model_name));
    assert_eq!(Some(format!("#/components/schemas/{model_name}")), reference.get_ref());
}

fn request_schema(open_api: &OpenAPI, path: &str) -> Schema {
    let body = open_api.get_paths().unwrap().get(path).unwrap().get_post().unwrap().get_request_body().unwrap();
    content_schema(&body.get_content(), "application/json").unwrap()
}

fn parameter_schema(open_api: &OpenAPI) -> Schema {
    open_api.get_paths().unwrap().get("/parameter").unwrap().get_get().unwrap().get_parameters().unwrap().get(0).unwrap().get_schema().unwrap()
}

fn json_response_schema(responses: &ApiResponses, code: &str) -> Schema {
    content_schema(&responses.get(code).unwrap().get_content(), "application/json").unwrap()
}

#[test]
fn test_dotted_request_body_object_uses_local_schema_ref() {
    let _g = common::setup(common::V3_MODULE);
    let schema = dotted_object_schema("admin.request");
    let open_api = OpenAPI::new().path("/request", PathItem::new().post(Operation::new().request_body(request_body(schema))));
    resolver().flatten(&open_api).unwrap();
    assert_local_schema_ref(&open_api, "admin.request", &request_schema(&open_api, "/request"));
}

#[test]
fn test_dotted_request_body_array_item_uses_local_schema_ref_when_reused() {
    let _g = common::setup(common::V3_MODULE);
    let item = dotted_object_schema("admin.item");
    let open_api = OpenAPI::new()
        .path("/first", PathItem::new().post(Operation::new().request_body(request_body(Schema::array_schema().items(item.clone())))))
        .path("/second", PathItem::new().post(Operation::new().request_body(request_body(Schema::array_schema().items(item)))));
    resolver().flatten(&open_api).unwrap();
    assert_local_schema_ref(&open_api, "admin.item", &request_schema(&open_api, "/first").get_items().unwrap());
    assert_local_schema_ref(&open_api, "admin.item", &request_schema(&open_api, "/second").get_items().unwrap());
}

#[test]
fn test_dotted_parameter_object_uses_local_schema_ref() {
    let _g = common::setup(common::V3_MODULE);
    let parameter = Parameter::new().name("filter").schema(dotted_object_schema("admin.parameter"));
    let open_api = open_api_with_parameter(parameter);
    resolver().flatten(&open_api).unwrap();
    assert_local_schema_ref(&open_api, "admin.parameter", &parameter_schema(&open_api));
}

#[test]
fn test_dotted_parameter_composed_schema_uses_local_schema_ref() {
    let _g = common::setup(common::V3_MODULE);
    let schema = Schema::composed_schema();
    schema.set_title("admin.parameter.composed");
    schema.add_one_of_item(Schema::string_schema());
    let open_api = open_api_with_parameter(Parameter::new().name("filter").schema(schema));
    resolver().flatten(&open_api).unwrap();
    assert_local_schema_ref(&open_api, "admin.parameter.composed", &parameter_schema(&open_api));
}

#[test]
fn test_dotted_parameter_array_item_uses_local_schema_ref() {
    let _g = common::setup(common::V3_MODULE);
    let parameter = Parameter::new().name("filter").schema(Schema::array_schema().items(dotted_object_schema("admin.parameter.item")));
    let open_api = open_api_with_parameter(parameter);
    resolver().flatten(&open_api).unwrap();
    assert_local_schema_ref(&open_api, "admin.parameter.item", &parameter_schema(&open_api).get_items().unwrap());
}

#[test]
fn test_dotted_parameter_array_item_uses_existing_local_schema_ref() {
    let _g = common::setup(common::V3_MODULE);
    let item = dotted_object_schema("admin.parameter.item");
    let parameter = Parameter::new().name("filter").schema(Schema::array_schema().items(item.clone()));
    let operation = Operation::new().request_body(request_body(Schema::array_schema().items(item))).add_parameters_item(parameter.clone());
    let open_api = OpenAPI::new().path("/parameter-array", PathItem::new().post(operation.clone()));
    resolver().flatten(&open_api).unwrap();
    let body_schema = content_schema(&operation.get_request_body().unwrap().get_content(), "application/json").unwrap();
    assert_local_schema_ref(&open_api, "admin.parameter.item", &body_schema.get_items().unwrap());
    assert_local_schema_ref(&open_api, "admin.parameter.item", &parameter.get_schema().unwrap().get_items().unwrap());
}

#[test]
fn test_dotted_response_array_item_uses_local_schema_ref() {
    let _g = common::setup(common::V3_MODULE);
    let item = dotted_object_schema("admin.response.item");
    let responses = ApiResponses::new()
        .add_api_response("200", response(Schema::array_schema().items(item.clone())))
        .add_api_response("201", response(Schema::array_schema().items(item)));
    let open_api = OpenAPI::new().path("/response", PathItem::new().get(Operation::new().responses(responses.clone())));
    resolver().flatten(&open_api).unwrap();
    assert_local_schema_ref(&open_api, "admin.response.item", &json_response_schema(&responses, "200").get_items().unwrap());
    assert_local_schema_ref(&open_api, "admin.response.item", &json_response_schema(&responses, "201").get_items().unwrap());
}

#[test]
fn test_dotted_response_object_uses_existing_local_schema_ref() {
    let _g = common::setup(common::V3_MODULE);
    let schema = dotted_object_schema("admin.response");
    let responses = ApiResponses::new().add_api_response("200", response(schema.clone())).add_api_response("201", response(schema));
    let open_api = OpenAPI::new().path("/response", PathItem::new().get(Operation::new().responses(responses.clone())));
    resolver().flatten(&open_api).unwrap();
    assert_local_schema_ref(&open_api, "admin.response", &json_response_schema(&responses, "200"));
    assert_local_schema_ref(&open_api, "admin.response", &json_response_schema(&responses, "201"));
}

fn additional_properties_schema(s: &Schema) -> Schema {
    s.get_additional_properties().unwrap().as_schema().unwrap().clone()
}

#[test]
fn test_dotted_response_map_uses_local_schema_ref() {
    let _g = common::setup(common::V3_MODULE);
    let map = Schema::object_schema();
    map.set_additional_properties(dotted_object_schema("admin.value"));
    let open_api = open_api_with_response_schema(map);
    resolver().flatten(&open_api).unwrap();
    let responses = open_api.get_paths().unwrap().get("/response").unwrap().get_get().unwrap().get_responses().unwrap();
    let flattened_map = json_response_schema(&responses, "200");
    assert_local_schema_ref(&open_api, "admin.value", &additional_properties_schema(&flattened_map));
}

#[test]
fn test_dotted_response_map_uses_existing_local_schema_ref() {
    let _g = common::setup(common::V3_MODULE);
    let value = dotted_object_schema("admin.value");
    let map = Schema::object_schema();
    map.set_additional_properties(value.clone());
    let operation = Operation::new().request_body(request_body(value)).responses(ApiResponses::new().add_api_response("200", response(map)));
    let open_api = OpenAPI::new().path("/response-map", PathItem::new().post(operation.clone()));
    resolver().flatten(&open_api).unwrap();
    let flattened_map = json_response_schema(&operation.get_responses().unwrap(), "200");
    assert_local_schema_ref(&open_api, "admin.value", &additional_properties_schema(&flattened_map));
}

#[test]
fn test_dotted_component_array_item_uses_local_schema_ref_when_reused() {
    let _g = common::setup(common::V3_MODULE);
    let item = dotted_object_schema("admin.component.item");
    let components = Components::new()
        .add_schemas("FirstArray", Schema::array_schema().items(item.clone()))
        .add_schemas("SecondArray", Schema::array_schema().items(item));
    let open_api = OpenAPI::new().components(components.clone());
    resolver().flatten(&open_api).unwrap();
    let s = components.get_schemas().unwrap();
    assert_local_schema_ref(&open_api, "admin.component.item", &s.get("FirstArray").unwrap().get_items().unwrap());
    assert_local_schema_ref(&open_api, "admin.component.item", &s.get("SecondArray").unwrap().get_items().unwrap());
}

#[test]
fn test_dotted_component_composed_child_uses_local_schema_ref() {
    let _g = common::setup(common::V3_MODULE);
    let child = dotted_object_schema("admin.composed");
    let composed = Schema::composed_schema();
    composed.add_all_of_item(child);
    let open_api = OpenAPI::new().components(Components::new().add_schemas("Wrapper", composed.clone()));
    resolver2(true, false).flatten(&open_api).unwrap();
    assert_local_schema_ref(&open_api, "admin.composed", &composed.get_all_of().unwrap().get(0).unwrap());
}

#[test]
fn test_dotted_request_composed_child_uses_local_schema_ref() {
    let _g = common::setup(common::V3_MODULE);
    let child = dotted_object_schema("admin.request.composed");
    let composed = Schema::composed_schema();
    composed.set_title("RequestWrapper");
    composed.add_any_of_item(child);
    let open_api = OpenAPI::new().path("/request-composed", PathItem::new().post(Operation::new().request_body(request_body(composed.clone()))));
    resolver2(true, false).flatten(&open_api).unwrap();
    assert_local_schema_ref(&open_api, "admin.request.composed", &composed.get_any_of().unwrap().get(0).unwrap());
}

#[test]
fn test_external_request_body_ref_is_not_rewritten() {
    let _g = common::setup(common::V3_MODULE);
    let external = Schema::new().ref_("./models/Product.yaml");
    let open_api = OpenAPI::new().path("/external", PathItem::new().post(Operation::new().request_body(request_body(external))));
    resolver().flatten(&open_api).unwrap();
    assert_eq!(Some("./models/Product.yaml".to_string()), request_schema(&open_api, "/external").get_ref());
}

/// The alpha/beta request schemas of `testSkipInlineMatches*`.
fn skip_inline_matches_open_api() -> OpenAPI {
    let open_api = OpenAPI::new();
    for op in ["Alpha", "Beta"] {
        let asset = Schema::object_schema();
        asset.set_title(format!("operation{op}InAsset").as_str());
        asset.add_properties("id1", Schema::integer_schema());
        asset.add_properties("id2", Schema::integer_schema());
        let in_ = Schema::object_schema();
        in_.set_title(format!("operation{op}In").as_str());
        in_.add_properties("asset", asset);
        let request = Schema::object_schema();
        request.set_title(format!("operation{op}Request").as_str());
        request.add_properties("in", in_);
        open_api.path(
            &format!("/operation{op}"),
            PathItem::new().get(Operation::new().request_body(RequestBody::new().content(Content::new().add_media_type("*/*", MediaType::new().schema(request))))),
        );
    }
    open_api
}

#[test]
fn test_skip_inline_matches_false() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = skip_inline_matches_open_api();
    let inline_model_resolver = resolver();
    inline_model_resolver.flatten(&open_api).unwrap();
    assert!(open_api.get_components().is_some());
    assert!(open_api.get_components().unwrap().get_schemas().is_some());
    assert_eq!(schemas(&open_api).len(), 6);
}

#[test]
fn test_skip_inline_matches_true() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = skip_inline_matches_open_api();
    let inline_model_resolver = InlineModelResolver::new(false, false, true);
    inline_model_resolver.flatten(&open_api).unwrap();
    assert!(open_api.get_components().is_some());
    assert!(open_api.get_components().unwrap().get_schemas().is_some());
    assert_eq!(6, schemas(&open_api).len());
}

fn user_array_schema(title: Option<&str>) -> Schema {
    let object_schema = address_object_schema(title);
    let array_schema = Schema::array_schema();
    array_schema.set_required(JList::from_vec(vec!["name".to_string()]));
    array_schema.set_items(object_schema);
    array_schema
}

#[test]
fn resolve_inline_array_model_with_title() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPI::new();
    open_api.set_components(Components::new());
    open_api.get_components().unwrap().add_schemas("User", user_array_schema(Some("InnerUserTitle")));
    resolver().flatten(&open_api).unwrap();
    assert!(schema_named(&open_api, "User").unwrap().is_array_schema());
    let user = schema_named(&open_api, "InnerUserTitle").unwrap();
    assert_eq!(Some("description".to_string()), user.get_description());
}

#[test]
fn resolve_inline_array_model_without_title() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPI::new();
    open_api.set_components(Components::new());
    open_api.get_components().unwrap().add_schemas("User", user_array_schema(None));
    resolver().flatten(&open_api).unwrap();
    assert!(schema_named(&open_api, "User").unwrap().is_array_schema());
    let user = schema_named(&open_api, "User_inner").unwrap();
    assert_eq!(Some("description".to_string()), user.get_description());
}

/// `address` object + `name` string schema used as a request body / parameter
fn address_name_schema() -> Schema {
    let object_schema = Schema::object_schema();
    object_schema.add_properties("street", Schema::string_schema());
    let schema = Schema::new();
    schema.add_properties("address", object_schema);
    schema.add_properties("name", Schema::string_schema());
    schema
}

fn open_api_with_get_body(path: &str, schema: Schema) -> OpenAPI {
    let media_type = MediaType::new();
    media_type.set_schema(schema);
    let content = Content::new();
    content.add_media_type("*/*", media_type);
    let request_body = RequestBody::new();
    request_body.set_content(content);
    let operation = Operation::new();
    operation.set_request_body(request_body);
    let path_item = PathItem::new();
    path_item.set_get(operation);
    OpenAPI::new().path(path, path_item)
}

fn get_body_schema(open_api: &OpenAPI, path: &str) -> Schema {
    let body = open_api.get_paths().unwrap().get(path).unwrap().get_get().unwrap().get_request_body().unwrap();
    content_schema(&body.get_content(), "*/*").unwrap()
}

#[test]
fn resolve_inline_request_body() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = open_api_with_get_body("/hello", address_name_schema());
    resolver().flatten(&open_api).unwrap();
    assert!(get_body_schema(&open_api, "/hello").get_ref().is_some());
    let body_schema = schema_named(&open_api, "hello_body").unwrap();
    assert!(prop(&body_schema, "address").is_some());
}

#[test]
fn resolve_inline_request_body_max_two_path_parts() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = open_api_with_get_body("/api/cloud/greet/hello", address_name_schema());
    resolver().flatten(&open_api).unwrap();
    assert!(get_body_schema(&open_api, "/api/cloud/greet/hello").get_ref().is_some());
    let body_schema = schema_named(&open_api, "greet_hello_body").unwrap();
    assert!(prop(&body_schema, "address").is_some());
}

#[test]
fn resolve_inline_request_body_strips_dots_from_path() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = open_api_with_get_body("/api/Cloud.Greet.Hello", address_name_schema());
    resolver2(true, true).flatten(&open_api).unwrap();
    assert_eq!(
        Some("#/components/schemas/ApiCloudGreetHelloBody".to_string()),
        get_body_schema(&open_api, "/api/Cloud.Greet.Hello").get_ref(),
        "use dot as common word separator: as it occurs frequently on OData services"
    );
    let body_schema = schema_named(&open_api, "ApiCloudGreetHelloBody").unwrap();
    assert!(prop(&body_schema, "address").is_some());
}

#[test]
fn resolve_inline_parameter() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPI::new();
    let parameter = Parameter::new();
    parameter.set_name("name");
    parameter.set_schema(address_name_schema());
    let operation = Operation::new();
    operation.set_parameters(JList::from_vec(vec![parameter]));
    let path_item = PathItem::new();
    path_item.set_get(operation);
    open_api.path("/hello", path_item);

    resolver().flatten(&open_api).unwrap();

    let param = open_api.get_paths().unwrap().get("/hello").unwrap().get_get().unwrap().get_parameters().unwrap().get(0).unwrap();
    assert!(param.get_schema().unwrap().get_ref().is_some());
    let body_schema = schema_named(&open_api, "name").unwrap();
    assert!(prop(&body_schema, "address").is_some());
}

#[test]
fn resolve_inline_request_body_with_title() {
    let _g = common::setup(common::V3_MODULE);
    let object_schema = Schema::object_schema();
    object_schema.add_properties("street", Schema::string_schema());
    object_schema.add_properties("name", Schema::string_schema());
    let address_model_item = Schema::new();
    let address_model_name = "DetailedAddress";
    address_model_item.set_title(address_model_name);
    address_model_item.add_properties("address", object_schema);
    let open_api = OpenAPI::new().path(
        "/hello",
        PathItem::new().get(Operation::new().request_body(RequestBody::new().content(Content::new().add_media_type("*/*", MediaType::new().schema(address_model_item))))),
    );

    resolver().flatten(&open_api).unwrap();

    assert!(get_body_schema(&open_api, "/hello").get_ref().is_some());
    let body = schema_named(&open_api, address_model_name).unwrap();
    assert!(prop(&body, "address").is_some());
}

#[test]
fn not_resolve_non_model_request_body() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = open_api_with_get_body("/hello", Schema::new().type_("string").format("binary"));
    resolver().flatten(&open_api).unwrap();
    let schema = get_body_schema(&open_api, "/hello");
    assert_eq!(Some("string".to_string()), schema.get_type());
    assert_eq!(Some("binary".to_string()), schema.get_format());
}

#[test]
fn resolve_inline_array_request_body() {
    let _g = common::setup(common::V3_MODULE);
    let address_schema = Schema::object_schema();
    address_schema.add_properties("street", Schema::string_schema());
    let object_schema = Schema::object_schema();
    object_schema.add_properties("address", address_schema);
    let array_schema = Schema::array_schema();
    array_schema.items(object_schema);
    let open_api = open_api_with_get_body("/hello", array_schema);

    resolver().flatten(&open_api).unwrap();

    let schema = get_body_schema(&open_api, "/hello");
    assert!(schema.is_array_schema());
    let inner = schema.get_items().unwrap();
    assert!(inner.get_ref().is_some());
    assert_eq!(Some("#/components/schemas/hello_body".to_string()), inner.get_ref());

    let inline = schema_named(&open_api, "hello_body").unwrap();
    let address = prop(&inline, "address").unwrap();
    assert_eq!(Some("#/components/schemas/hello_address".to_string()), address.get_ref());

    let inline_prop = schema_named(&open_api, "hello_address").unwrap();
    assert!(prop(&inline_prop, "street").is_some());
    assert!(prop(&inline_prop, "street").unwrap().is_kind(SchemaKind::String));
}

fn get_response(open_api: &OpenAPI, path: &str) -> ApiResponse {
    open_api.get_paths().unwrap().get(path).unwrap().get_get().unwrap().get_responses().unwrap().get("200").unwrap()
}

#[test]
fn resolve_inline_array_response() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPI::new();
    let items = Schema::object_schema();
    items.add_extension("x-ext", JavaObject::string("ext-items"));
    items.add_properties("name", Schema::string_schema());
    let schema = Schema::array_schema().items(items);
    schema.add_extension("x-ext", JavaObject::string("ext-prop"));
    let response = ApiResponse::new();
    response.add_extension("x-foo", JavaObject::string("bar"));
    response.description("it works!");
    response.content(Content::new().add_media_type("*/*", MediaType::new().schema(schema)));
    open_api.path("/foo/baz", PathItem::new().get(Operation::new().responses(ApiResponses::new().add_api_response("200", response))));

    resolver().flatten(&open_api).unwrap();

    let api_response = get_response(&open_api, "/foo/baz");
    let response_property = content_schema(&api_response.get_content(), "*/*").unwrap();
    // no need to flatten more
    assert!(response_property.is_array_schema());
    assert_eq!(1, response_property.get_extensions().unwrap().len());
    assert_eq!(Some(JavaObject::string("ext-prop")), ext(&response_property, "x-ext"));

    let p = response_property.get_items().unwrap();
    assert_eq!(Some("#/components/schemas/inline_response_200".to_string()), p.get_ref());
    assert_eq!(1, p.get_extensions().unwrap().len());
    assert_eq!(Some(JavaObject::string("ext-items")), ext(&p, "x-ext"));

    let inline = schema_named(&open_api, "inline_response_200").unwrap();
    assert!(prop(&inline, "name").is_some());
    assert!(prop(&inline, "name").unwrap().is_kind(SchemaKind::String));
}

#[test]
fn resolve_inline_array_response_with_title() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPI::new();
    let api_response = ApiResponse::new();
    api_response.add_extension("x-foo", JavaObject::string("bar"));
    api_response.description("it works!");
    let properties = swagger_parser::java::JMap::new_hash();
    properties.put("name", Schema::string_schema());
    api_response.content(Content::new().add_media_type(
        "*/*",
        MediaType::new().schema(Schema::array_schema().items(Schema::object_schema().title("FooBar").properties(properties))),
    ));
    open_api.path("/foo/baz", PathItem::new().get(Operation::new().responses(ApiResponses::new().add_api_response("200", api_response))));

    resolver().flatten(&open_api).unwrap();

    let response = get_response(&open_api, "/foo/baz");
    let response_property = content_schema(&response.get_content(), "*/*").unwrap();
    // no need to flatten more
    assert!(response_property.is_array_schema());
    let p = response_property.get_items().unwrap();
    assert_eq!(p.get_ref(), Some("#/components/schemas/FooBar".to_string()));

    let inline = schema_named(&open_api, "FooBar").unwrap();
    assert!(prop(&inline, "name").is_some());
    assert!(prop(&inline, "name").unwrap().is_kind(SchemaKind::String));
}

#[test]
fn test_inline_map_response() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPI::new();
    let schema = Schema::new();
    schema.set_additional_properties(Schema::string_schema());
    schema.add_extension("x-ext", JavaObject::string("ext-prop"));
    let api_response = ApiResponse::new();
    api_response.description("it works!");
    let media_type = MediaType::new();
    media_type.set_schema(schema);
    let content = Content::new();
    content.add_media_type("*/*", media_type);
    api_response.set_content(content);
    api_response.add_extension("x-foo", JavaObject::string("bar"));
    let api_responses = ApiResponses::new();
    api_responses.add_api_response("200", api_response);
    open_api.path("/foo/baz", PathItem::new().get(Operation::new().responses(api_responses)));

    resolver().flatten(&open_api).unwrap();

    let response = get_response(&open_api, "/foo/baz");
    let property = content_schema(&response.get_content(), "*/*").unwrap();
    assert!(property.get_additional_properties().is_some());
    assert!(open_api.get_components().unwrap().get_schemas().is_none());
    assert_eq!(1, property.get_extensions().unwrap().len());
    assert_eq!(Some(JavaObject::string("ext-prop")), ext(&property, "x-ext"));
}

#[test]
fn test_inline_map_response_with_object_schema() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPI::new();
    let schema = Schema::new();
    schema.set_additional_properties(Schema::object_schema().add_properties("name", Schema::string_schema()));
    schema.add_extension("x-ext", JavaObject::string("ext-prop"));
    let api_response = ApiResponse::new().description("it works!").content(Content::new().add_media_type("*/*", MediaType::new().schema(schema)));
    api_response.add_extension("x-foo", JavaObject::string("bar"));
    let api_responses = ApiResponses::new().add_api_response("200", api_response);
    open_api.path("/foo/baz", PathItem::new().get(Operation::new().responses(api_responses)));

    resolver().flatten(&open_api).unwrap();

    let response = get_response(&open_api, "/foo/baz");
    let property = content_schema(&response.get_content(), "*/*").unwrap();
    assert!(property.get_additional_properties().is_some());
    assert_eq!(1, property.get_extensions().unwrap().len());
    assert_eq!(Some(JavaObject::string("ext-prop")), ext(&property, "x-ext"));
    assert!(schemas(&open_api).len() == 1);

    let inline = schema_named(&open_api, "inline_response_map200").unwrap();
    assert!(prop(&inline, "name").is_some());
    assert!(prop(&inline, "name").unwrap().is_kind(SchemaKind::String));
}

#[test]
fn test_array_response() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPI::new();
    let object_schema = Schema::object_schema();
    object_schema.add_properties("name", Schema::string_schema());
    let schema = Schema::array_schema();
    schema.set_items(object_schema);
    let api_response = ApiResponse::new();
    api_response.add_extension("x-foo", JavaObject::string("bar"));
    api_response.set_description("it works!");
    api_response.set_content(Content::new().add_media_type("*/*", MediaType::new().schema(schema)));
    open_api.path("/foo/baz", PathItem::new().get(Operation::new().responses(ApiResponses::new().add_api_response("200", api_response))));

    resolver().flatten(&open_api).unwrap();

    let response = get_response(&open_api, "/foo/baz");
    let am = content_schema(&response.get_content(), "*/*").unwrap();
    assert!(am.is_array_schema());
    let items = am.get_items().unwrap();
    assert!(items.get_ref().is_some());
    assert_eq!(items.get_ref(), Some("#/components/schemas/inline_response_200".to_string()));

    let inline = schema_named(&open_api, "inline_response_200").unwrap();
    assert!(prop(&inline, "name").is_some());
    assert!(prop(&inline, "name").unwrap().is_kind(SchemaKind::String));
}

#[test]
fn test_basic_input() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPI::new();
    open_api.set_components(Components::new());
    let user = Schema::new();
    user.add_properties("name", Schema::string_schema());
    open_api.path(
        "/foo/baz",
        PathItem::new().post(Operation::new().request_body(RequestBody::new().content(Content::new().add_media_type("*/*", MediaType::new().schema(Schema::new().ref_("User")))))),
    );
    open_api.get_components().unwrap().add_schemas("User", user);
    resolver().flatten(&open_api).unwrap();
}

#[test]
fn test_arbitrary_request_body() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = open_api_with_get_body("/hello", Schema::new());
    resolver().flatten(&open_api).unwrap();
    let schema = get_body_schema(&open_api, "/hello");
    assert!(schema.get_type().is_none());
}

#[test]
fn test_arbitrary_object_request_body_inline() {
    let _g = common::setup(common::V3_MODULE);
    let schema = Schema::new();
    schema.add_properties("arbitrary", Schema::object_schema());
    let swagger = open_api_with_get_body("/hello", schema);

    resolver().flatten(&swagger).unwrap();

    assert!(get_body_schema(&swagger, "/hello").get_ref().is_some());
    let body = schema_named(&swagger, "hello_body").unwrap();
    let property = prop(&body, "arbitrary").unwrap();
    assert!(property.is_object_schema());
}

#[test]
fn test_arbitrary_object_body_param_with_array() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = open_api_with_get_body("/hello", Schema::array_schema().items(Schema::object_schema()));
    resolver().flatten(&open_api).unwrap();
    let schema = get_body_schema(&open_api, "/hello");
    assert!(schema.is_array_schema());
    let inner = schema.get_items().unwrap();
    assert!(inner.is_object_schema());
    assert!(inner.get_properties().is_none());
}

#[test]
fn test_arbitrary_object_body_param_array_inline() {
    let _g = common::setup(common::V3_MODULE);
    let items = Schema::object_schema();
    items.add_properties("arbitrary", Schema::object_schema());
    let open_api = open_api_with_get_body("/hello", Schema::array_schema().items(items));
    resolver().flatten(&open_api).unwrap();
    let schema = get_body_schema(&open_api, "/hello");
    assert!(schema.is_array_schema());
    let inner = schema.get_items().unwrap();
    assert!(inner.get_ref().is_some());
    assert_eq!(inner.get_ref(), Some("#/components/schemas/hello_body".to_string()));
    let inline = schema_named(&open_api, "hello_body").unwrap();
    let p = prop(&inline, "arbitrary").unwrap();
    assert!(p.is_object_schema());
}

fn open_api_with_response(path: &str, schema: Schema) -> OpenAPI {
    OpenAPI::new().path(
        path,
        PathItem::new().get(Operation::new().responses(
            ApiResponses::new().add_api_response("200", ApiResponse::new().description("it works!").content(Content::new().add_media_type("*/*", MediaType::new().schema(schema)))),
        )),
    )
}

#[test]
fn test_arbitrary_object_response() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = open_api_with_response("/foo/bar", Schema::object_schema());
    resolver().flatten(&open_api).unwrap();
    let response = get_response(&open_api, "/foo/bar");
    let op = content_schema(&response.get_content(), "*/*").unwrap();
    assert!(op.is_object_schema());
    assert!(op.get_properties().is_none());
}

#[test]
fn test_arbitrary_object_response_array() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = open_api_with_response("/foo/baz", Schema::array_schema().items(Schema::object_schema()));
    resolver().flatten(&open_api).unwrap();
    let response = get_response(&open_api, "/foo/baz");
    let array_schema = content_schema(&response.get_content(), "*/*").unwrap();
    assert!(array_schema.is_array_schema());
    let items = array_schema.get_items().unwrap();
    assert!(items.is_object_schema());
    assert!(items.get_properties().is_none());
}

#[test]
fn test_arbitrary_object_response_array_inline() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPI::new();
    let array_schema = Schema::array_schema();
    let object_schema = Schema::object_schema();
    object_schema.add_properties("arbitrary", Schema::object_schema());
    array_schema.items(object_schema);
    let api_response = ApiResponse::new();
    api_response.add_extension("x-foo", JavaObject::string("bar"));
    api_response.description("it works!");
    api_response.content(Content::new().add_media_type("*/*", MediaType::new().schema(array_schema)));
    open_api.path("/foo/baz", PathItem::new().get(Operation::new().responses(ApiResponses::new().add_api_response("200", api_response))));

    resolver().flatten(&open_api).unwrap();

    let response = get_response(&open_api, "/foo/baz");
    let response_property = content_schema(&response.get_content(), "*/*").unwrap();
    assert!(response_property.is_array_schema());
    let items = response_property.get_items().unwrap();
    assert_eq!(Some("#/components/schemas/inline_response_200".to_string()), items.get_ref());

    let inline = schema_named(&open_api, "inline_response_200").unwrap();
    let inline_prop = prop(&inline, "arbitrary").unwrap();
    assert!(inline_prop.is_object_schema());
    assert!(inline_prop.get_properties().is_none());
}

#[test]
fn test_arbitrary_object_response_map_inline() {
    let _g = common::setup(common::V3_MODULE);
    let schema = Schema::new();
    schema.set_additional_properties(Schema::object_schema());
    let open_api = open_api_with_response("/foo/baz", schema);

    resolver().flatten(&open_api).unwrap();

    let response = get_response(&open_api, "/foo/baz");
    let property = content_schema(&response.get_content(), "*/*").unwrap();
    assert!(property.get_additional_properties().is_some());
    assert!(property.get_additional_properties().unwrap().as_schema().is_some());
    assert!(open_api.get_components().unwrap().get_schemas().is_none());
    let inline_prop = additional_properties_schema(&property);
    assert!(inline_prop.is_object_schema());
    assert!(inline_prop.get_properties().is_none());
}

#[test]
fn test_arbitrary_object_model_inline() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPI::new();
    open_api.set_components(Components::new());
    let user_schema = Schema::new();
    user_schema.set_name("user");
    user_schema.set_description("a common user");
    user_schema.add_properties("name", Schema::string_schema());
    let object_schema = Schema::object_schema();
    object_schema.set_title("title");
    object_schema.set_default("default");
    object_schema.set_read_only(false);
    object_schema.set_description("description");
    object_schema.set_name("name");
    user_schema.add_properties("arbitrary", object_schema);
    user_schema.set_required(JList::from_vec(vec!["arbitrary".to_string()]));
    open_api.get_components().unwrap().add_schemas("User", user_schema);

    resolver().flatten(&open_api).unwrap();

    let user = schema_named(&open_api, "User").unwrap();
    let inline_prop = prop(&user, "arbitrary").unwrap();
    assert!(inline_prop.is_object_schema());
    assert!(inline_prop.get_properties().is_none());
}

fn arbitrary_items(title: Option<&str>) -> Schema {
    let items = Schema::object_schema();
    if let Some(t) = title {
        items.set_title(t);
    }
    items.set_default("default");
    items.set_read_only(false);
    items.set_description("description");
    items.set_name("name");
    items.add_properties("arbitrary", Schema::object_schema());
    items
}

#[test]
fn test_arbitrary_object_model_with_array_inline_without_title() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPI::new();
    open_api.set_components(Components::new());
    open_api.get_components().unwrap().add_schemas("User", Schema::array_schema().items(arbitrary_items(None)).add_required_item("name"));

    resolver().flatten(&open_api).unwrap();

    let model = schema_named(&open_api, "User").unwrap();
    assert!(model.is_array_schema());
    assert!(model.get_items().unwrap().get_ref().is_some());
    let user_inner = schema_named(&open_api, "User_inner").unwrap();
    let inline_prop = prop(&user_inner, "arbitrary").unwrap();
    assert!(inline_prop.is_object_schema());
    assert!(inline_prop.get_properties().is_none());
}

#[test]
fn test_arbitrary_object_model_with_array_inline_with_title() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPI::new();
    open_api.set_components(Components::new());
    open_api.get_components().unwrap().add_schemas("User", Schema::array_schema().items(arbitrary_items(Some("InnerUserTitle"))).add_required_item("name"));

    resolver().flatten(&open_api).unwrap();

    let model = schema_named(&open_api, "User").unwrap();
    assert!(model.is_array_schema());
    assert!(model.get_items().unwrap().get_ref().is_some());
    let user_inner = schema_named(&open_api, "InnerUserTitle").unwrap();
    let inline_prop = prop(&user_inner, "arbitrary").unwrap();
    assert!(inline_prop.is_object_schema());
    assert!(inline_prop.get_properties().is_none());
}

/// https://github.com/swagger-api/swagger-parser/issues/1527
#[test]
fn test_inline_items_schema() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = read_flatten("flatten.json", false).expect("openAPI");
    assert!(schema_named(&open_api, "inline_response_200").is_some());
}

/// `Integer.getInteger(name)`: the system properties "0", "1", "10", "100" are not set, so every call
/// yields `null`.
fn integer_get_integer(_name: &str) -> Option<i32> {
    None
}

/// https://github.com/swagger-api/swagger-parser/issues/1200
#[test]
fn test_schema_properties_being_passed_to_flattened_model() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPI::new();
    open_api.set_components(Components::new());

    let address = Schema::object_schema();
    address.set_deprecated(false);
    address.set_description("My address");
    address.set_exclusive_maximum(true);
    address.set_exclusive_minimum(true);
    address.set_format("format");
    address.set_min_length(integer_get_integer("10"));
    address.set_maximum(BigDecimal::from_i64(50));
    address.set_max_items(integer_get_integer("1"));
    address.set_max_length(integer_get_integer("100"));
    address.set_max_properties(integer_get_integer("1"));
    address.set_minimum(BigDecimal::from_i64(0));
    address.set_min_items(integer_get_integer("0"));
    address.set_min_length(integer_get_integer("10"));
    address.set_min_properties(integer_get_integer("0"));
    address.set_multiple_of(BigDecimal::from_i64(2));
    address.set_name("Address");
    address.set_nullable(true);
    address.set_pattern("%dd");
    address.set_read_only(false);
    address.set_title("my address");
    address.set_unique_items(true);
    address.set_write_only(false);
    address.add_properties("city", Schema::string_schema());

    let user = Schema::object_schema();
    user.set_title("InnerUserTitle");
    user.set_default("default");
    user.set_read_only(false);
    user.set_description("user description");
    user.set_name("user name");
    user.add_properties("address", address);
    open_api.get_components().unwrap().add_schemas("User", user);

    resolver2(true, true).flatten(&open_api).unwrap();

    assert!(schema_named(&open_api, "User").unwrap().is_object_schema());
    let user_address = schema_named(&open_api, "MyAddress").unwrap();
    assert_eq!(user_address.get_deprecated(), Some(false));
    assert_eq!(user_address.get_description(), Some("My address".to_string()));
    assert_eq!(user_address.get_exclusive_maximum(), Some(true));
    assert_eq!(user_address.get_exclusive_minimum(), Some(true));
    assert_eq!(user_address.get_format(), Some("format".to_string()));
    assert_eq!(user_address.get_maximum(), Some(BigDecimal::from_i64(50)));
    assert_eq!(user_address.get_max_items(), integer_get_integer("1"));
    assert_eq!(user_address.get_max_length(), integer_get_integer("100"));
    assert_eq!(user_address.get_max_properties(), integer_get_integer("1"));
    assert_eq!(user_address.get_minimum(), Some(BigDecimal::from_i64(0)));
    assert_eq!(user_address.get_min_items(), integer_get_integer("1"));
    assert_eq!(user_address.get_min_length(), integer_get_integer("100"));
    assert_eq!(user_address.get_min_properties(), integer_get_integer("0"));
    assert_eq!(user_address.get_multiple_of(), Some(BigDecimal::from_i64(2)));
    assert_eq!(user_address.get_name(), Some("Address".to_string()));
    assert_eq!(user_address.get_nullable(), Some(true));
    assert_eq!(user_address.get_pattern(), Some("%dd".to_string()));
    assert_eq!(user_address.get_read_only(), Some(false));
    assert_eq!(user_address.get_title(), Some("my address".to_string()));
    assert_eq!(user_address.get_unique_items(), Some(true));
    assert_eq!(user_address.get_write_only(), Some(false));
}
