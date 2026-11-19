//! Port of `io.swagger.v3.parser.test.OpenAPIResolverTest` (the class-wide WireMock server on a dynamic
//! port → one local stub server per test that needs it).

mod common;

use common::wiremock::*;
use jackson::yaml::read::{read_tree, YamlReadFeatures};
use jackson::JavaObject;
use swagger_parser::core::{AuthorizationValue, ParseOptions};
use swagger_parser::java::JList;
use swagger_parser::models::*;
use swagger_parser::parser::openapi_resolver::Settings;
use swagger_parser::parser::reference::{dereference_31, DereferencerContext};
use swagger_parser::parser::resolver_fully::ResolverFully;
use swagger_parser::parser::{OpenAPIDeserializer, OpenAPIResolver, OpenAPIV3Parser};

const REMOTE_REF_JSON: &str = "http://localhost:${dynamicPort}/remote_ref_json#/components/schemas/Tag";
const REMOTE_REF_YAML: &str = "http://localhost:${dynamicPort}/remote_ref_yaml#/components/schemas/Tag";

/// `List<AuthorizationValue> auths = new ArrayList<>()`
const AUTHS: &[AuthorizationValue] = &[];

/// `@BeforeClass setUpWireMockServer`
fn set_up_wire_mock_server() -> WireMockServer {
    let server = WireMockServer::start(0, false);
    let port = server.port().to_string();
    let stub = |path: &str, file: &str, content_type: &str, replace_port: bool| {
        let mut body = common::read_file(&format!("src/test/resources/remote_references/{file}"));
        if replace_port {
            body = body.replace("${dynamicPort}", &port);
        }
        server.stub_for(get(url_path_matching(path)).will_return(a_response().with_status(200).with_header("Content-type", content_type).with_body(body.into_bytes())));
    };
    stub("/remote/path", "remote_pathItem.yaml.template", "application/yaml", false);
    stub("/remote/schema", "remote_schema.yaml", "application/yaml", false);
    stub("/remote/response", "remote_responses.yaml.template", "application/yaml", true);
    stub("/remote/requestBody", "remote_requestBody.yaml", "application/yaml", false);
    stub("/remote/parameter", "remote_parameter.yaml.template", "application/yaml", true);
    stub("/remote/example", "remote_example.yaml", "application/yaml", false);
    stub("/remote/link", "remote_link.yaml", "application/yaml", false);
    stub("/remote/callback", "remote_callback.yaml", "application/yaml", false);
    // Java stubs /remote/link a second time with the callback file's content (the later stub wins in WireMock)
    stub("/remote/link", "remote_callback.yaml", "application/yaml", false);
    stub("/remote/security", "remote_securityScheme.yaml", "application/yaml", false);
    stub("/remote_ref_yaml", "remote_ref_yaml.yaml", "application/yaml", false);
    stub("/remote_ref_json", "remote_ref_json.json", "application/json", false);
    server
}

fn replace_port(url: &str, server: &WireMockServer) -> String {
    url.replace("${dynamicPort}", &server.port().to_string())
}

/// `new ObjectMapper(new YAMLFactory()).readTree(template with the port)` + `new OpenAPIDeserializer().deserialize(rootNode)`
fn deserialize_template(file: &str, server: &WireMockServer) -> swagger_parser::core::SwaggerParseResult {
    let path_file = common::read_file(file).replace("${dynamicPort}", &server.port().to_string());
    let root_node = read_tree(&path_file, YamlReadFeatures::swagger_core()).unwrap();
    OpenAPIDeserializer::new().deserialize(&root_node)
}

fn opts(resolve: bool, resolve_fully: bool) -> ParseOptions {
    let mut o = ParseOptions::new();
    o.resolve = resolve;
    o.resolve_fully = resolve_fully;
    o
}

fn read_location(location: &str, auths: Option<&[AuthorizationValue]>, options: &ParseOptions) -> Option<OpenAPI> {
    OpenAPIV3Parser::new().read_location(location, auths, Some(options)).openapi
}

fn read(location: &str, auths: Option<&[AuthorizationValue]>, options: &ParseOptions) -> Option<OpenAPI> {
    OpenAPIV3Parser::new().read_with(Some(location), auths, Some(options))
}

fn read_contents(yaml: &str, auths: Option<&[AuthorizationValue]>, options: Option<&ParseOptions>) -> Option<OpenAPI> {
    OpenAPIV3Parser::new().read_contents(yaml, auths, options).openapi
}

fn s(v: &str) -> Option<String> {
    Some(v.to_string())
}

fn schemas(open_api: &OpenAPI) -> swagger_parser::java::JMap<Schema> {
    open_api.get_components().unwrap().get_schemas().unwrap()
}

fn prop(schema: &Schema, name: &str) -> Schema {
    schema.get_properties().unwrap().get(name).unwrap()
}

fn media_schema(content: Option<Content>, media: &str) -> Schema {
    content.unwrap().get(media).unwrap().get_schema().unwrap()
}

fn path(open_api: &OpenAPI, p: &str) -> PathItem {
    open_api.get_paths().unwrap().get(p).unwrap()
}

fn response(op: &Operation, code: &str) -> ApiResponse {
    op.get_responses().unwrap().get(code).unwrap()
}

#[test]
fn components_resolver31() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up_wire_mock_server();
    let mut result = deserialize_template("src/test/resources/oas31.yaml.template", &server);
    let open_api = result.openapi.clone().expect("openAPI");
    let dereferencer_context = DereferencerContext::new(&result, Some(&[]), None, &ParseOptions::new());
    // DereferencersFactory: the registered CustomOpenAPIDereferencer (an OpenAPIDereferencer31 with a custom
    // readHttp, installed by common::setup) handles a 3.1 document.
    dereference_31(&mut result, Some(&[]), None, &ParseOptions::new()).unwrap();
    assert_eq!(dereferencer_context.open_api, Some(open_api.clone()));

    let schemas = schemas(&open_api);

    //internal url schema
    let pet = schemas.get("Pet").unwrap();
    let category = prop(&pet, "category");
    assert_eq!(category.get_ref(), s("#/components/schemas/Category"));

    //remote url schema
    let _user = prop(&pet, "user");
    // TODO uncomment when bundling of extenrnal refs gets implemented
    // assertEquals(user.get$ref(),"#/components/schemas/User");

    //ArraySchema items
    let _tags_property = prop(&pet, "tags");

    //Schema not
    assert_eq!(schemas.get("OrderRef").unwrap().get_not().unwrap().get_ref(), s("#/components/schemas/Category"));

    //Schema additionalProperties
    assert!(schemas.get("OrderRef").unwrap().get_additional_properties().unwrap().as_schema().is_some());

    //AllOfSchema
    let extended = schemas.get("ExtendedErrorModel").unwrap();
    let root = prop(&extended.get_all_of().unwrap().get(0).unwrap(), "rootCause");
    assert_eq!(root.get_ref(), s("#/components/schemas/Category"));

    let responses = open_api.get_components().unwrap().get_responses().unwrap();

    //internal response headers
    let illegal_input = responses.get("IllegalInput").unwrap();
    assert_eq!(illegal_input.get_headers().unwrap().get("X-Ref-Limit-Limit").unwrap().get_ref(), s("#/components/headers/X-Rate-Limit-Reset"));

    //internal response links
    assert_eq!(illegal_input.get_links().unwrap().get("address").unwrap().get_ref(), s("#/components/links/unsubscribe"));

    //internal url response schema
    let general_error = media_schema(responses.get("GeneralError").unwrap().get_content(), "application/json");
    assert_eq!(general_error.get_ref(), s("#/components/schemas/ExtendedErrorModel"));

    let request_bodies = open_api.get_components().unwrap().get_request_bodies().unwrap();

    //internal url requestBody schema
    let xml_media = media_schema(request_bodies.get("requestBody1").unwrap().get_content(), "application/json");
    assert_eq!(xml_media.get_ref(), s("#/components/schemas/Pet"));

    //internal url requestBody ArraySchema
    let _items = media_schema(request_bodies.get("requestBody2").unwrap().get_content(), "application/json");

    //internal request body
    assert_eq!(s("#/components/requestBodies/requestBody2"), request_bodies.get("requestBody3").unwrap().get_ref());

    let parameters = open_api.get_components().unwrap().get_parameters().unwrap();

    //internal Schema Parameter
    assert_eq!(parameters.get("newParam").unwrap().get_schema().unwrap().get_ref(), s("#/components/schemas/Tag"));

    //parameter content schema
    assert_eq!(media_schema(parameters.get("contentParameter").unwrap().get_content(), "application/json").get_ref(), s("#/components/schemas/ExtendedErrorModel"));

    let headers = open_api.get_components().unwrap().get_headers().unwrap();
    //header examples
    assert_eq!(headers.get("X-Rate-Limit-Reset").unwrap().get_examples().unwrap().get("headerExample").unwrap().get_ref(), s("#/components/examples/dog"));
    //header content
    assert_eq!(media_schema(headers.get("X-Rate-Limit-Reset").unwrap().get_content(), "application/json").get_ref(), s("#/components/schemas/ExtendedErrorModel"));

    let examples = open_api.get_components().unwrap().get_examples().unwrap();
    //internal url example
    assert_eq!(examples.get("frog").unwrap().get_ref(), s("#/components/examples/cat"));

    let _remote_scheme = open_api.get_components().unwrap().get_security_schemes().unwrap().get("remote_reference");

    //internal link
    assert_eq!(open_api.get_components().unwrap().get_links().unwrap().get("referenced").unwrap().get_ref(), s("#/components/links/unsubscribe"));

    let callbacks = open_api.get_components().unwrap().get_callbacks().unwrap();
    // internal callback reference
    assert_eq!(callbacks.get("referenced").unwrap().get_ref(), s("#/components/callbacks/failed"));
    //callback pathItem -> operation ->requestBody
    assert_eq!(
        callbacks.get("heartbeat").unwrap().get("$request.query.heartbeat-url").unwrap().get_post().unwrap().get_request_body().unwrap().get_ref(),
        s("#/components/requestBodies/requestBody3")
    );
}

#[test]
fn components_resolver() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up_wire_mock_server();
    let result = deserialize_template("src/test/resources/oas3.yaml.template", &server);
    let open_api = result.openapi.clone().expect("openAPI");
    assert_eq!(OpenAPIResolver::with_location(Some(&open_api), Some(&[]), None).unwrap().resolve().unwrap(), Some(open_api.clone()));

    let schemas = schemas(&open_api);

    //internal url schema
    let pet = schemas.get("Pet").unwrap();
    assert_eq!(prop(&pet, "category").get_ref(), s("#/components/schemas/Category"));

    //remote url schema
    assert_eq!(prop(&pet, "user").get_ref(), s("#/components/schemas/User"));

    //ArraySchema items
    let tags_property = prop(&pet, "tags");
    assert!(tags_property.is_array_schema());
    assert_eq!(tags_property.get_items().unwrap().get_ref(), s("#/components/schemas/ExampleSchema"));
    assert_eq!(tags_property.get_type(), s("array"));
    assert!(schemas.get("ExampleSchema").is_some());

    //Schema not
    assert_eq!(schemas.get("OrderRef").unwrap().get_not().unwrap().get_ref(), s("#/components/schemas/Category"));

    //Schema additionalProperties
    let additional_properties = schemas.get("OrderRef").unwrap().get_additional_properties().unwrap();
    assert!(additional_properties.as_schema().is_some());
    assert_eq!(additional_properties.as_schema().unwrap().get_ref(), s("#/components/schemas/User"));

    //AllOfSchema
    let extended = schemas.get("ExtendedErrorModel").unwrap();
    assert!(extended.is_composed_schema());
    let root = prop(&extended.get_all_of().unwrap().get(0).unwrap(), "rootCause");
    assert_eq!(root.get_ref(), s("#/components/schemas/Category"));

    let components = open_api.get_components().unwrap();
    let responses = components.get_responses().unwrap();

    //internal response headers
    let illegal_input = responses.get("IllegalInput").unwrap();
    assert_eq!(illegal_input.get_headers().unwrap().get("X-Ref-Limit-Limit").unwrap().get_ref(), s("#/components/headers/X-Rate-Limit-Reset"));

    //internal response links
    assert_eq!(illegal_input.get_links().unwrap().get("address").unwrap().get_ref(), s("#/components/links/unsubscribe"));

    //internal url response schema
    assert_eq!(media_schema(responses.get("GeneralError").unwrap().get_content(), "application/json").get_ref(), s("#/components/schemas/ExtendedErrorModel"));

    let request_bodies = components.get_request_bodies().unwrap();

    //internal url requestBody schema
    assert_eq!(media_schema(request_bodies.get("requestBody1").unwrap().get_content(), "application/json").get_ref(), s("#/components/schemas/Pet"));

    //internal url requestBody ArraySchema
    let items = media_schema(request_bodies.get("requestBody2").unwrap().get_content(), "application/json");
    assert!(items.is_array_schema());
    assert_eq!(items.get_items().unwrap().get_ref(), s("#/components/schemas/User"));

    //internal request body
    assert_eq!(s("#/components/requestBodies/requestBody2"), request_bodies.get("requestBody3").unwrap().get_ref());

    //remote request body url
    assert_eq!(request_bodies.get("reference").unwrap().get_ref(), s("#/components/requestBodies/remote_requestBody"));

    let parameters = components.get_parameters().unwrap();

    //remote url parameter
    assert_eq!(parameters.get("remoteParameter").unwrap().get_ref(), s("#/components/parameters/parameter"));

    //internal Schema Parameter
    assert_eq!(parameters.get("newParam").unwrap().get_schema().unwrap().get_ref(), s("#/components/schemas/Tag"));

    //parameter examples
    assert_eq!(parameters.get("contentParameter").unwrap().get_examples().unwrap().get("cat"), components.get_examples().unwrap().get("cat"));

    //parameter content schema
    assert_eq!(media_schema(parameters.get("contentParameter").unwrap().get_content(), "application/json").get_ref(), s("#/components/schemas/ExtendedErrorModel"));

    let headers = components.get_headers().unwrap();
    //header remote schema ref
    assert_eq!(headers.get("X-Rate-Limit-Remaining").unwrap().get_schema().unwrap().get_ref(), s("#/components/schemas/User"));

    //header examples
    assert_eq!(headers.get("X-Rate-Limit-Reset").unwrap().get_examples().unwrap().get("headerExample").unwrap().get_ref(), s("#/components/examples/dog"));
    //remote header ref
    assert_eq!(headers.get("X-Ref-Limit-Limit").unwrap().get_ref(), s("#/components/headers/X-Rate-Limit-Reset"));

    //header content
    assert_eq!(media_schema(headers.get("X-Rate-Limit-Reset").unwrap().get_content(), "application/json").get_ref(), s("#/components/schemas/ExtendedErrorModel"));

    let examples = components.get_examples().unwrap();

    //internal url example
    assert_eq!(examples.get("frog").unwrap().get_ref(), s("#/components/examples/cat"));

    //remote example url
    assert_eq!(examples.get("referenceCat").unwrap().get_ref(), s("#/components/examples/example"));

    //internal url securityScheme
    let scheme = components.get_security_schemes().unwrap().get("reference").unwrap();
    assert_eq!(scheme.get_type(), Some(SecuritySchemeType::ApiKey));

    let remote_scheme = components.get_security_schemes().unwrap().get("remote_reference").unwrap();
    assert_eq!(remote_scheme.get_type(), Some(SecuritySchemeType::Oauth2));

    //internal link
    assert_eq!(components.get_links().unwrap().get("referenced").unwrap().get_ref(), s("#/components/links/unsubscribe"));
    //remote ref link
    assert_eq!(components.get_links().unwrap().get("subscribe").unwrap().get_ref(), s("#/components/links/link"));

    let callbacks = components.get_callbacks().unwrap();
    // internal callback reference
    assert_eq!(callbacks.get("referenced").unwrap().get_ref(), s("#/components/callbacks/failed"));
    //callback pathItem -> operation ->requestBody
    assert_eq!(
        callbacks.get("heartbeat").unwrap().get("$request.query.heartbeat-url").unwrap().get_post().unwrap().get_request_body().unwrap().get_ref(),
        s("#/components/requestBodies/requestBody3")
    );
    //remote callback ref
    assert_eq!(callbacks.get("remoteCallback").unwrap().get_ref(), s("#/components/callbacks/callback"));
}

#[test]
fn paths_resolver() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up_wire_mock_server();
    let result = deserialize_template("src/test/resources/oas3.yaml.template", &server);
    let open_api = result.openapi.clone().expect("openAPI");

    assert_eq!(OpenAPIResolver::with_location(Some(&open_api), Some(&[]), None).unwrap().resolve().unwrap(), Some(open_api.clone()));

    //internal url pathItem
    assert_eq!(open_api.get_paths().unwrap().get("/pathItemRef2"), open_api.get_paths().unwrap().get("/pet"));

    //internal array schema inside operation -> responses -> content
    let schema = media_schema(response(&path(&open_api, "/pet").get_put().unwrap(), "400").get_content(), "application/json");
    assert!(schema.is_array_schema());
    assert_eq!(schema.get_items().unwrap().get_ref(), s("#/components/schemas/VeryComplexType"));

    //replace of parameters in operation and remove the ones from the pathItem
    assert!(path(&open_api, "/pet").get_post().unwrap().get_parameters().is_some());
    assert!(path(&open_api, "/pet").get_parameters().is_none());

    //remote ref pathItem
    assert_eq!(path(&open_api, "/pathItemRef").get_summary(), s("summary"));
    assert_eq!(response(&path(&open_api, "/pathItemRef").get_post().unwrap(), "405").get_description(), s("Invalid input"));

    //internal pathItem operation -> response -> schema
    let pet_get = path(&open_api, "/pet/{petId}").get_get().unwrap();
    assert!(pet_get.get_responses().is_some());
    assert_eq!(media_schema(response(&pet_get, "200").get_content(), "application/xml").get_ref(), s("#/components/schemas/Pet"));

    //internal pathItem -> operation -> callback -> pathItem -> operation -> response -> schema
    let cb_op = pet_get.get_callbacks().unwrap().get("mainHook").unwrap().get("$request.body#/url").unwrap().get_post().unwrap();
    assert_eq!(media_schema(response(&cb_op, "200").get_content(), "application/xml").get_ref(), s("#/components/schemas/Pet"));

    //internal pathItem -> operation -> requestBody
    let body = path(&open_api, "/pet/findByStatus").get_get().unwrap().get_request_body().unwrap();
    let id = prop(&media_schema(body.get_content(), "multipart/mixed"), "id");
    assert_eq!(id.get_ref(), s("#/components/schemas/Pet"));

    //internal parameter url
    assert_eq!(
        path(&open_api, "/store/inventory").get_get().unwrap().get_parameters().unwrap().get(0),
        open_api.get_components().unwrap().get_parameters().unwrap().get("limitParam")
    );
}

#[test]
fn test_self_reference_resolution() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = "".to_string()
        + "openapi: 3.0.1\n"
        + "paths:\n"
        + "  \"/selfRefB\":\n"
        + "    get:\n"
        + "      requestBody:\n"
        + "        description: user to add to the system\\n\"+\n"
        + "        content:\n"
        + "         'application/json':\n"
        + "             schema:\n"
        + "                $ref: \"#/components/schemas/SchemaB\"\n"
        + "components:\n"
        + "  schemas:\n"
        + "    SchemaA:\n"
        + "      properties:\n"
        + "        name:\n"
        + "          type: string\n"
        + "        modelB:\n"
        + "          $ref: \"#/components/schemas/SchemaB\"\n"
        + "    SchemaB:\n"
        + "      properties:\n"
        + "        modelB:\n"
        + "          $ref: \"#/components/schemas/SchemaB\"";

    let options = opts(true, true);
    let open_api = read_contents(&yaml, Some(AUTHS), Some(&options)).unwrap();
    let mut resolver_util = ResolverFully::new(&options);
    resolver_util.resolve_fully(Some(&open_api)).unwrap();

    let body = path(&open_api, "/selfRefB").get_get().unwrap().get_request_body().unwrap();
    let schema = media_schema(body.get_content(), "application/json");
    assert_eq!(Some(schema), schemas(&open_api).get("SchemaB"));
}

#[test]
fn test_issue85() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = "openapi: '3.0.1'\n".to_string()
        + "paths: \n"
        + "  /test/method: \n"
        + "    post: \n"
        + "      parameters: \n"
        + "        - \n"
        + "          in: \"path\"\n"
        + "          name: \"body\"\n"
        + "          required: false\n"
        + "          schema: \n"
        + "            $ref: \"#/components/Schemas/StructureA\"\n"
        + "components: \n"
        + "   schemas:\n"
        + "       StructureA: \n"
        + "           type: object\n"
        + "           properties: \n"
        + "               someProperty: \n"
        + "                   type: string\n"
        + "               arrayOfOtherType: \n"
        + "                   type: array\n"
        + "                   items: \n"
        + "                       $ref: \"#/definitions/StructureB\"\n"
        + "       StructureB: \n"
        + "           type: object\n"
        + "           properties: \n"
        + "               someProperty: \n"
        + "                   type: string\n";

    let options = opts(true, true);
    let open_api = read_contents(&yaml, Some(AUTHS), Some(&options)).unwrap();
    let mut resolver_util = ResolverFully::new(&options);
    resolver_util.resolve_fully(Some(&open_api)).unwrap();
    let param = path(&open_api, "/test/method").get_post().unwrap().get_parameters().unwrap().get(0).unwrap();

    let schema = param.get_schema().unwrap();
    assert!(schema.get_properties().unwrap().get("someProperty").is_some());

    let am = schema.get_properties().unwrap().get("arrayOfOtherType").expect("arrayOfOtherType");
    assert!(am.is_array_schema());
    // `assertTrue(prop instanceof Schema)`: items is non-null
    assert!(am.get_items().is_some());
}

#[test]
fn test_issue1352() {
    let _g = common::setup(common::V3_MODULE);
    let options = opts(true, true);
    let open_api = read_location("issue-1352.json", Some(AUTHS), &options).unwrap();
    let patch = path(&open_api, "/responses").get_patch().unwrap();
    assert!(response(&patch, "200").get_headers().unwrap().get("x-my-secret-header").unwrap().get_schema().unwrap().get_ref().is_none());
}

#[test]
fn test_issue1157() {
    let _g = common::setup(common::V3_MODULE);
    let options = opts(true, true);

    let open_api_any_of = read_location("/issue-1157/anyOf-example.yaml", Some(AUTHS), &options).unwrap();
    let pet_schema_any_of = schemas(&open_api_any_of).get("Pet").unwrap();
    assert!(pet_schema_any_of.is_composed_schema());
    assert!(pet_schema_any_of.get_any_of().is_some());

    let open_api_one_of = read_location("/issue-1157/oneOf-example.yaml", Some(AUTHS), &options).unwrap();
    let pet_schema_one_of = schemas(&open_api_one_of).get("Pet").unwrap();
    assert!(pet_schema_one_of.is_composed_schema());
    assert!(pet_schema_one_of.get_one_of().is_some());

    let open_api_all_of = read_location("/issue-1157/allOf-example.yaml", Some(AUTHS), &options).unwrap();
    let pet_schema_all_of = schemas(&open_api_all_of).get("Pet").unwrap();
    assert!(!pet_schema_all_of.is_composed_schema());
    assert!(pet_schema_all_of.get_properties().is_some());
}

#[test]
fn test_issue1161() {
    let _g = common::setup(common::V3_MODULE);
    let options = opts(true, true);
    let open_api = read_location("/issue-1161/swagger.yaml", Some(AUTHS), &options).unwrap();

    let pets_schema = schemas(&open_api).get("Pets").expect("Pets");
    let colourings_schema = schemas(&open_api).get("Colouring").expect("Colouring");

    assert!(pets_schema.is_composed_schema());
    assert!(pets_schema.get_properties().is_some());
    assert!(pets_schema.get_one_of().is_some());

    let pets_colouring_property = prop(&pets_schema, "colouring");
    assert!(pets_colouring_property.get_ref().is_none());
    assert!(pets_colouring_property.ptr_eq(&colourings_schema));
}

#[test]
fn test_issue1170() {
    let _g = common::setup(common::V3_MODULE);
    let options = opts(true, true);
    let open_api = read_location("/issue-1170/swagger.yaml", Some(AUTHS), &options).unwrap();

    // Array schema with items $ref
    let breeds_list_schema = schemas(&open_api).get("BreedsList").expect("BreedsList");
    let breed_schema = schemas(&open_api).get("Breed").expect("Breed");

    assert!(breeds_list_schema.is_array_schema());
    let breed_property_schema = prop(&breeds_list_schema.get_items().unwrap(), "breed");

    // Verify items resolved fully
    assert!(breed_property_schema.get_ref().is_none());
    assert!(breed_property_schema.ptr_eq(&breed_schema));

    // Array schema with inline items object with $ref properties
    let pets_list_schema = schemas(&open_api).get("PetsList").expect("PetsList");
    let colourings_schema = schemas(&open_api).get("Colouring").expect("Colouring");
    assert!(schemas(&open_api).get("Colour").is_some());

    assert!(pets_list_schema.is_array_schema());
    let colouring_property_schema = prop(&pets_list_schema.get_items().unwrap(), "colouring");

    // Verify inline items resolved fully
    assert!(colouring_property_schema.get_ref().is_none());
    assert!(colouring_property_schema.ptr_eq(&colourings_schema));
}

#[test]
fn test_issue1706() {
    let _g = common::setup(common::V3_MODULE);
    let mut options = ParseOptions::new();
    options.resolve = true;
    // Added this parseOption to make requestBody/response inline in the operation resolve processing flow.
    options.resolve_request_body = true;
    options.resolve_responses = true;

    let open_api = read_location("/issue-1706/SimpleRequestResponseRef.json", None, &options).unwrap();
    let post = path(&open_api, "/resource").get_post().unwrap();

    // RequestBody should be inline
    let body = post.get_request_body().unwrap();
    assert!(body.get_ref().is_none());
    assert!(body.get_content().is_some());
    assert!(media_schema(body.get_content(), "application/json").is_object_schema());

    // Responses should be inline
    let r = response(&post, "200");
    assert!(r.get_ref().is_none());
    assert!(r.get_content().is_some());
    assert!(media_schema(r.get_content(), "application/json").is_object_schema());
}

#[test]
fn self_reference_test() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = "".to_string()
        + "openapi: 3.0.1\n"
        + "paths:\n"
        + "  /selfRefA:\n"
        + "    get:\n"
        + "      requestBody:\n"
        + "        content:\n"
        + "          application/json:\n"
        + "            schema:\n"
        + "              $ref: \"#/components/schemas/ModelA\"\n"
        + "      responses:\n"
        + "        default:\n"
        + "          description: Default response\n"
        + "  /selfRefB:\n"
        + "    get:\n"
        + "      requestBody:\n"
        + "        content:\n"
        + "          application/json:\n"
        + "            schema:\n"
        + "              $ref: \"#/components/schemas/ModelB\"\n"
        + "      responses:\n"
        + "        default:\n"
        + "          description: Default response\n"
        + "  /selfRefC:\n"
        + "    get:\n"
        + "      requestBody:\n"
        + "        content:\n"
        + "          application/json:\n"
        + "            schema:\n"
        + "              $ref: \"#/components/schemas/ModelC\"\n"
        + "      responses:\n"
        + "        default:\n"
        + "          description: Default response\n"
        + "  /selfRefD:\n"
        + "    get:\n"
        + "      responses:\n"
        + "        default:\n"
        + "          description: Default response\n"
        + "          content:\n"
        + "            '*/*':\n"
        + "              schema:\n"
        + "                $ref: \"#/components/schemas/ModelA\"\n"
        + "  /selfRefE:\n"
        + "    get:\n"
        + "      responses:\n"
        + "        default:\n"
        + "          description: Default response\n"
        + "          content:\n"
        + "            '*/*':\n"
        + "              schema:\n"
        + "                type: array\n"
        + "                items:\n"
        + "                  $ref: \"#/components/schemas/ModelA\"\n"
        + "info:\n"
        + "  version: ''\n"
        + "  title: ''\n"
        + "components:\n"
        + "  schemas:\n"
        + "    ModelA:\n"
        + "      properties:\n"
        + "        modelB:\n"
        + "          $ref: \"#/components/schemas/ModelB\"\n"
        + "    ModelB:\n"
        + "      properties:\n"
        + "        modelB:\n"
        + "          $ref: \"#/components/schemas/ModelB\"\n"
        + "    ModelC:\n"
        + "      properties:\n"
        + "        modelA:\n"
        + "          $ref: \"#/components/schemas/ModelA\"";

    let options = opts(true, true);
    let open_api = read_contents(&yaml, Some(AUTHS), Some(&options)).unwrap();

    let schema_b = media_schema(path(&open_api, "/selfRefB").get_get().unwrap().get_request_body().unwrap().get_content(), "application/json");
    assert_eq!(schemas(&open_api).get("ModelB"), Some(schema_b));

    let schema = media_schema(response(&path(&open_api, "/selfRefE").get_get().unwrap(), "default").get_content(), "*/*");
    assert!(schema.is_array_schema());
    assert_eq!(schemas(&open_api).get("ModelA"), schema.get_items());
}

#[test]
fn resolve_all_of_without_aggregating_parameters() {
    let _g = common::setup(common::V3_MODULE);
    let mut options = ParseOptions::new();
    options.resolve_fully = true;
    options.resolve_combinators = false;

    // Testing components/schemas
    let open_api = read_location("src/test/resources/composed.yaml", Some(AUTHS), &options).unwrap();

    let all_of = schemas(&open_api).get("ExtendedAddress").unwrap();
    assert!(all_of.is_composed_schema());
    assert_eq!(all_of.get_all_of().unwrap().len(), 2);
    assert!(all_of.get_all_of().unwrap().get(0).unwrap().get_properties().unwrap().contains_key("street"));
    assert!(all_of.get_all_of().unwrap().get(1).unwrap().get_properties().unwrap().contains_key("gps"));

    // Testing path item
    let schema = media_schema(path(&open_api, "/withInvalidComposedModel").get_post().unwrap().get_request_body().unwrap().get_content(), "application/json");
    assert!(schema.is_composed_schema());

    // In fact the schema resolved previously is the same of /withInvalidComposedModel
    assert_eq!(schema, all_of);

    assert_eq!(schema.get_all_of().unwrap().len(), 2);
    assert!(schema.get_all_of().unwrap().get(0).unwrap().get_properties().unwrap().contains_key("street"));
    assert!(schema.get_all_of().unwrap().get(1).unwrap().get_properties().unwrap().contains_key("gps"));
}

#[test]
fn resolve_composed_reference_all_of_schema() {
    let _g = common::setup(common::V3_MODULE);
    let options = opts(true, true);
    let open_api = read_location("src/test/resources/composed.yaml", Some(AUTHS), &options).unwrap();

    let array_schema = media_schema(path(&open_api, "/withInvalidComposedModelArray").get_post().unwrap().get_request_body().unwrap().get_content(), "application/json");
    assert!(array_schema.is_array_schema());
    assert!(array_schema.get_items().unwrap().is_object_schema());
}

#[test]
fn resolve_composed_schema() {
    let _g = common::setup(common::V3_MODULE);
    let mut options = ParseOptions::new();
    options.resolve_combinators = false;
    options.resolve_fully = true;
    let open_api = read_location("src/test/resources/oneof-anyof.yaml", Some(AUTHS), &options).unwrap();

    let array_schema = media_schema(response(&path(&open_api, "/mixed-array").get_get().unwrap(), "200").get_content(), "application/json");
    assert!(array_schema.is_array_schema());
    let one_of = array_schema.get_items().unwrap();
    assert!(one_of.is_composed_schema());
    assert_eq!(one_of.get_one_of().unwrap().get(0).unwrap().get_type(), s("string"));

    let one_of_schema = media_schema(response(&path(&open_api, "/oneOf").get_get().unwrap(), "200").get_content(), "application/json");
    assert!(one_of_schema.is_composed_schema());
    assert_eq!(one_of_schema.get_one_of().unwrap().get(0).unwrap().get_type(), s("object"));
}

#[test]
fn referring_spec_without_components_tag() {
    let _g = common::setup(common::V3_MODULE);
    let mut resolve = ParseOptions::new();
    resolve.resolve_fully = true;
    let open_api = read("./ref-without-component/a.yaml", None, &resolve).unwrap();
    let schemas = schemas(&open_api);
    assert_eq!(Some(JavaObject::string("Example value")), schemas.get("CustomerType").unwrap().get_example());
}

#[test]
fn test_ref_name_conflicts() {
    let _g = common::setup(common::V3_MODULE);
    let mut options = ParseOptions::new();
    options.resolve_fully = true;
    let open_api = read_location("/refs-name-conflict/a.yaml", None, &options).unwrap();

    let location_example = |p: &str| prop(&media_schema(response(&path(&open_api, p).get_post().unwrap(), "200").get_content(), "*/*"), "location").get_example();
    let ex = |v: &str| Some(JavaObject::string(v));
    assert_eq!(ex("local"), location_example("/newPerson"));
    assert_eq!(ex("referred"), location_example("/oldPerson"));
    assert_eq!(ex("referred"), location_example("/yetAnotherPerson"));
    assert_eq!(ex("local"), prop(&schemas(&open_api).get("PersonObj").unwrap(), "location").get_example());
    assert_eq!(ex("referred"), prop(&schemas(&open_api).get("PersonObj_1").unwrap(), "location").get_example());
    assert_eq!(ex("referred-again"), prop(&schemas(&open_api).get("PersonObj_2").unwrap(), "location").get_example());
}

#[test]
fn test_parameter_on_path_level() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = "openapi: 3.0.1\n".to_string()
        + "info:\n"
        + "  version: '1.0.0'\n"
        + "  title: 'title'\n"
        + "  description: 'description'\n"
        + "paths:\n"
        + "  /foo:\n"
        + "    parameters:\n"
        + "      - in: query\n"
        + "        name: bar\n"
        + "        schema:\n"
        + "          type: string\n"
        + "    get:\n"
        + "      responses:\n"
        + "        '200':\n"
        + "          description: OK";
    let mut options = ParseOptions::new();
    options.resolve = true;
    let open_api = read_contents(&yaml, None, Some(&options)).expect("openAPI");
    let get_parameters = path(&open_api, "/foo").get_get().unwrap().get_parameters().expect("parameters");
    assert_eq!(1, get_parameters.len());
    assert_eq!(s("bar"), get_parameters.get(0).unwrap().get_name());
}

#[test]
fn test_composed_schema_adjacent() {
    let _g = common::setup(common::V3_MODULE);
    let options = opts(true, true);
    let open_api = read("src/test/resources/composedSchemaRef.yaml", Some(AUTHS), &options).expect("openAPI");

    assert!(schemas(&open_api).len() == 5);
    let schema = media_schema(response(&path(&open_api, "/path").get_get().unwrap(), "200").get_content(), "application/json");
    assert!(schema.get_properties().unwrap().len() == 4);

    let schema_one_of = media_schema(response(&path(&open_api, "/oneOf").get_get().unwrap(), "200").get_content(), "application/json");
    assert!(schema_one_of.is_composed_schema());
    assert!(schema_one_of.get_one_of().unwrap().len() == 3);

    let schema_any_of = media_schema(response(&path(&open_api, "/anyOf").get_get().unwrap(), "200").get_content(), "application/json");
    assert!(schema_any_of.is_composed_schema());
    assert!(schema_any_of.get_any_of().unwrap().len() == 3);
}

#[test]
fn test_composed_schema_adjacent_with_examples() {
    let _g = common::setup(common::V3_MODULE);
    let options = opts(true, true);
    let open_api = read("src/test/resources/anyOf_OneOf.yaml", Some(AUTHS), &options).expect("openAPI");

    assert!(schemas(&open_api).len() == 2);

    let schema_path = media_schema(response(&path(&open_api, "/path").get_get().unwrap(), "200").get_content(), "application/json");
    assert!(schema_path.get_properties().unwrap().len() == 5);
    // `getExample() instanceof HashSet`
    let Some(JavaObject::Set(examples)) = schema_path.get_example() else { panic!("example is not a HashSet: {:?}", schema_path.get_example()) };
    assert!(examples.len() == 2);
}

#[test]
fn all_of_example_generation() {
    let _g = common::setup(common::V3_MODULE);
    let options = opts(true, true);
    let open_api = read("src/test/resources/simpleAllOf.yaml", None, &options).expect("openAPI");

    let without_example = media_schema(response(&path(&open_api, "/foo").get_get().unwrap(), "200").get_content(), "application/json").get_example();
    assert!(without_example.is_none());

    let with_example = media_schema(response(&path(&open_api, "/bar").get_get().unwrap(), "200").get_content(), "application/json").get_example();
    // Json.mapper().writeValueAsString(null) would be "null"
    let with_example = with_example.unwrap_or(JavaObject::Null);
    assert_eq!("{\"someProperty\":\"ABC\",\"someOtherProperty\":42}", json_string(&with_example).unwrap());
}

#[test]
fn test_swagger_resolver_null_swagger() {
    let _g = common::setup(common::V3_MODULE);
    assert!(OpenAPIResolver::with_location(None, None, None).unwrap().resolve().unwrap().is_none());
}

/// `new OpenAPIResolver(swagger, null).resolve()`
fn resolve2(swagger: &OpenAPI) -> OpenAPI {
    OpenAPIResolver::with_location(Some(swagger), None, None).unwrap().resolve().unwrap().unwrap()
}

fn test_simple_remote_model_property(remote_ref: &str) {
    let swagger = OpenAPI::new();
    swagger.components(Components::new().add_schemas("Sample", Schema::new().add_properties("remoteRef", Schema::new().ref_(remote_ref))));

    let resolved = resolve2(&swagger);
    let p = prop(&schemas(&resolved).get("Sample").unwrap(), "remoteRef");
    assert!(p.get_ref().is_some());
    assert_eq!(p.get_ref(), s("#/components/schemas/Tag"));
    assert!(schemas(&swagger).get("Tag").is_some());
}

/// resolve a simple remote model property definition in json
#[test]
fn test_json_simple_remote_model_property() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up_wire_mock_server();
    test_simple_remote_model_property(&replace_port(REMOTE_REF_JSON, &server));
}

/// resolve a simple remote model property definition in yaml
#[test]
fn test_yaml_simple_remote_model_property() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up_wire_mock_server();
    test_simple_remote_model_property(&replace_port(REMOTE_REF_YAML, &server));
}

fn test_array_remote_model_property(remote_ref: &str) {
    let swagger = OpenAPI::new();
    swagger.components(Components::new().add_schemas("Sample", Schema::new().add_properties("remoteRef", Schema::array_schema().items(Schema::new().ref_(remote_ref)))));

    let resolved = resolve2(&swagger);
    let p = prop(&schemas(&resolved).get("Sample").unwrap(), "remoteRef");
    assert!(p.is_array_schema());
    assert_eq!(p.get_items().unwrap().get_ref(), s("#/components/schemas/Tag"));
    assert!(schemas(&swagger).get("Tag").is_some());
}

/// resolve an array remote model property definition in json
#[test]
fn test_json_array_remote_model_property() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up_wire_mock_server();
    test_array_remote_model_property(&replace_port(REMOTE_REF_JSON, &server));
}

/// resolve an array remote model property definition in yaml
#[test]
fn test_yaml_array_remote_model_property() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up_wire_mock_server();
    test_array_remote_model_property(&replace_port(REMOTE_REF_YAML, &server));
}

fn test_map_remote_model_property(remote_ref: &str) {
    let swagger = OpenAPI::new();
    swagger.components(Components::new().add_schemas("Sample", Schema::new().add_properties("remoteRef", Schema::new().additional_properties(Schema::new().ref_(remote_ref)))));

    let resolved = resolve2(&swagger);
    let p = prop(&schemas(&resolved).get("Sample").unwrap(), "remoteRef");
    assert!(p.get_additional_properties().is_some());
    assert_eq!(p.get_additional_properties().unwrap().as_schema().unwrap().get_ref(), s("#/components/schemas/Tag"));
    assert!(schemas(&swagger).get("Tag").is_some());
}

/// resolve an map remote model property definition in json
#[test]
fn test_json_map_remote_model_property() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up_wire_mock_server();
    test_map_remote_model_property(&replace_port(REMOTE_REF_JSON, &server));
}

/// resolve an map remote model property definition in yaml
#[test]
fn test_yaml_map_remote_model_property() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up_wire_mock_server();
    test_map_remote_model_property(&replace_port(REMOTE_REF_YAML, &server));
}

fn test_operation_bodyparam_remote_refs(remote_ref: &str) {
    let swagger = OpenAPI::new();
    swagger.path(
        "/fun",
        PathItem::new().get(Operation::new().request_body(RequestBody::new().content(Content::new().add_media_type("*/*", MediaType::new().schema(Schema::new().ref_(remote_ref)))))),
    );

    let _resolved = resolve2(&swagger);
    let param = path(&swagger, "/fun").get_get().unwrap().get_request_body().unwrap();
    let r = media_schema(param.get_content(), "*/*");
    assert_eq!(r.get_ref(), s("#/components/schemas/Tag"));
    assert!(schemas(&swagger).get("Tag").is_some());
}

/// resolve operation bodyparam remote refs in json
#[test]
fn test_json_operation_bodyparam_remote_refs() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up_wire_mock_server();
    test_operation_bodyparam_remote_refs(&replace_port(REMOTE_REF_JSON, &server));
}

/// resolve operation bodyparam remote refs in yaml
#[test]
fn test_yaml_operation_bodyparam_remote_refs() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up_wire_mock_server();
    test_operation_bodyparam_remote_refs(&replace_port(REMOTE_REF_YAML, &server));
}

/// resolve operation parameter remote refs
#[test]
fn test_operation_parameter_remote_refs() {
    let _g = common::setup(common::V3_MODULE);
    let swagger = OpenAPI::new();
    let parameters = JList::from_vec(vec![Parameter::new().ref_("#/components/parameters/SampleParameter")]);
    swagger.path("/fun", PathItem::new().get(Operation::new().parameters(parameters)));
    swagger.components(Components::new().add_parameters("SampleParameter", Parameter::of(ParameterKind::Query).name("skip").schema(Schema::integer_schema())));

    let _resolved = resolve2(&swagger);

    let params = path(&swagger, "/fun").get_get().unwrap().get_parameters().unwrap();
    assert_eq!(params.len(), 1);
    assert_eq!(params.get(0).unwrap().get_name(), s("skip"));
}

#[test]
fn test_operation_body_parameter_remote_refs() {
    let _g = common::setup(common::V3_MODULE);
    let schema = Schema::new();
    let swagger = OpenAPI::new();
    swagger.path("/fun", PathItem::new().get(Operation::new().parameters(JList::from_vec(vec![Parameter::new().ref_("#/components/parameters/SampleParameter")]))));
    swagger.path("/times", PathItem::new().get(Operation::new().parameters(JList::from_vec(vec![Parameter::new().ref_("#/components/parameters/SampleParameter")]))));
    swagger.components(Components::new().add_parameters("SampleParameter", Parameter::new().name("skip").schema(schema)));

    let _resolved = resolve2(&swagger);
    let params = path(&swagger, "/fun").get_get().unwrap().get_parameters().unwrap();
    assert_eq!(params.len(), 1);
    assert_eq!(params.get(0).unwrap().get_name(), s("skip"));
}

fn test_response_remote_refs(remote_ref: &str) {
    let swagger = OpenAPI::new();
    swagger.path(
        "/fun",
        PathItem::new().get(Operation::new().responses(ApiResponses::new().add_api_response(
            "200",
            ApiResponse::new().content(Content::new().add_media_type("*/*", MediaType::new().schema(Schema::new().ref_(remote_ref)))),
        ))),
    );

    let _resolved = resolve2(&swagger);
    let r = response(&path(&swagger, "/fun").get_get().unwrap(), "200");
    assert_eq!(media_schema(r.get_content(), "*/*").get_ref(), s("#/components/schemas/Tag"));
    assert!(schemas(&swagger).get("Tag").is_some());
}

/// resolve response remote refs in json
#[test]
fn test_json_response_remote_refs() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up_wire_mock_server();
    test_response_remote_refs(&replace_port(REMOTE_REF_JSON, &server));
}

/// resolve response remote refs in yaml
#[test]
fn test_yaml_response_remote_refs() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up_wire_mock_server();
    test_response_remote_refs(&replace_port(REMOTE_REF_YAML, &server));
}

/// resolve array response remote refs in yaml
#[test]
fn test_yaml_array_response_remote_refs() {
    let _g = common::setup(common::V3_MODULE);
    let server = set_up_wire_mock_server();
    let swagger = OpenAPI::new();
    swagger.path(
        "/fun",
        PathItem::new().get(Operation::new().responses(ApiResponses::new().add_api_response(
            "200",
            ApiResponse::new().content(
                Content::new().add_media_type("*/*", MediaType::new().schema(Schema::array_schema().items(Schema::new().ref_(replace_port(REMOTE_REF_YAML, &server))))),
            ),
        ))),
    );

    let _resolved = resolve2(&swagger);
    let r = response(&path(&swagger, "/fun").get_get().unwrap(), "200");
    let array = media_schema(r.get_content(), "*/*");
    assert!(array.is_array_schema());
    assert!(array.get_items().is_some());
    assert_eq!(array.get_items().unwrap().get_ref(), s("#/components/schemas/Tag"));
    assert!(schemas(&swagger).get("Tag").is_some());
}

/// resolve shared path parameters
#[test]
fn test_shared_path_parameters_test() {
    let _g = common::setup(common::V3_MODULE);
    let swagger = OpenAPI::new();
    let operation = Operation::new().responses(ApiResponses::new().add_api_response("200", ApiResponse::new().description("ok!")));
    let p = PathItem::new().get(operation);
    p.add_parameters_item(Parameter::of(ParameterKind::Query).name("username").schema(Schema::string_schema()));
    swagger.path("/fun", p);

    let resolved = resolve2(&swagger);
    assert!(path(&resolved, "/fun").get_parameters().is_none());
    assert!(path(&resolved, "/fun").get_get().unwrap().get_parameters().unwrap().len() == 1);
}

/// resolve top-level parameters
#[test]
fn test_shared_swagger_parameters_test() {
    let _g = common::setup(common::V3_MODULE);
    let swagger = OpenAPI::new();
    let parameters = JList::new();
    parameters.insert(0, Parameter::new().ref_("username"));
    swagger.path(
        "/fun",
        PathItem::new().get(Operation::new().parameters(parameters).responses(ApiResponses::new().add_api_response("200", ApiResponse::new().description("ok!")))),
    );
    swagger.components(Components::new().add_parameters("username", Parameter::of(ParameterKind::Query).name("username").schema(Schema::string_schema())));

    let resolved = resolve2(&swagger);
    assert!(resolved.get_components().unwrap().get_parameters().unwrap().len() == 1);
    assert!(path(&resolved, "/fun").get_get().unwrap().get_parameters().unwrap().len() == 1);
}

/// resolve top-level responses
#[test]
fn test_shared_responses() {
    let _g = common::setup(common::V3_MODULE);
    let swagger = OpenAPI::new();
    let mut parse_options = ParseOptions::new();
    parse_options.resolve_responses = true;
    let parameters = JList::new();
    parameters.insert(0, Parameter::new().ref_("username"));
    swagger.path(
        "/fun",
        PathItem::new().get(Operation::new().parameters(parameters).responses(ApiResponses::new().add_api_response("200", ApiResponse::new().ref_("#/components/responses/foo")))),
    );
    swagger.components(Components::new().add_responses("foo", ApiResponse::new().description("ok!")));

    let resolved = OpenAPIResolver::with_all(Some(&swagger), None, None, None, parse_options).unwrap().resolve().unwrap().unwrap();
    let r = response(&path(&resolved, "/fun").get_get().unwrap(), "200");
    assert!(r.get_description().unwrap() == "ok!");
}

#[test]
fn test_issue291() {
    let _g = common::setup(common::V3_MODULE);
    let json = "openapi: 3.0.0\n".to_string()
        + "servers: []\n"
        + "info:\n"
        + "  title: test spec\n"
        + "  version: '1.0'\n"
        + "paths:\n"
        + "  /test:\n"
        + "    get:\n"
        + "      description: test get\n"
        + "      parameters:\n"
        + "        - $ref: \"#/components/parameters/testParam\"\n"
        + "      responses:\n"
        + "        default:\n"
        + "          description: test response\n"
        + "components:\n"
        + "  parameters:\n"
        + "    testParam:\n"
        + "      name: test\n"
        + "      in: query\n"
        + "      style: form\n"
        + "      schema:\n"
        + "        type: array\n"
        + "        items:\n"
        + "          type: string";
    let swagger = read_contents(&json, None, None).unwrap();
    let resolved = resolve2(&swagger);

    let param = path(&resolved, "/test").get_get().unwrap().get_parameters().unwrap().get(0).unwrap();
    // `(QueryParameter) param` must not throw ClassCastException
    assert_eq!(param.kind(), ParameterKind::Query);
}

#[test]
fn test_settings_add_parameters_to_each_operation_disabled() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = "openapi: 3.0.0\n".to_string()
        + "servers: []\n"
        + "info:\n"
        + "  title: test spec\n"
        + "  version: '1.0'\n"
        + "paths:\n"
        + "  '/test/{id}':\n"
        + "    parameters:\n"
        + "      - name: id\n"
        + "        in: path\n"
        + "        required: true\n"
        + "        schema:\n"
        + "          type: string\n"
        + "    get:\n"
        + "      description: test get\n"
        + "      parameters:\n"
        + "        - name: page\n"
        + "          in: query\n"
        + "          schema:\n"
        + "            type: string\n"
        + "      responses:\n"
        + "        default:\n"
        + "          description: test response";

    let swagger = read_contents(&yaml, None, None).unwrap();

    let resolved = OpenAPIResolver::with_all(Some(&swagger), None, None, Some(Settings::default().add_parameters_to_each_operation(false)), ParseOptions::new())
        .unwrap()
        .resolve()
        .unwrap()
        .unwrap();

    let p = path(&resolved, "/test/{id}");
    assert_eq!(p.get_parameters().unwrap().len(), 1);
    let pp = p.get_parameters().unwrap().get(0).unwrap();
    assert_eq!(pp.kind(), ParameterKind::Path);
    assert_eq!(pp.get_name(), s("id"));

    assert_eq!(p.get_get().unwrap().get_parameters().unwrap().len(), 1);
    let qp = p.get_get().unwrap().get_parameters().unwrap().get(0).unwrap();
    assert_eq!(qp.kind(), ParameterKind::Query);
    assert_eq!(qp.get_name(), s("page"));
}

/// update internal references of external files
#[test]
fn test_update_internal_references_of_external_files() {
    let _g = common::setup(common::V3_MODULE);
    let mut options = ParseOptions::new();
    options.resolve = true;
    let open_api = read("internal-references-in-external-files/main.yaml", None, &options).unwrap();

    let common_schema = schemas(&open_api).get("common").unwrap();
    assert!(common_schema.is_composed_schema());
    let all_of = common_schema.get_all_of().unwrap();
    assert_eq!(all_of.get(0).unwrap().get_ref(), s("#/components/schemas/core"));
    assert_eq!(prop(&all_of.get(1).unwrap(), "direct").get_ref(), s("#/components/schemas/core"));
    let referenced = prop(&all_of.get(1).unwrap(), "referenced");
    assert!(referenced.is_array_schema());
    assert_eq!(referenced.get_items().unwrap().get_ref(), s("#/components/schemas/core"));
    let core_schema = schemas(&open_api).get("core").unwrap();
    assert_eq!(prop(&core_schema, "inner").get_ref(), s("#/components/schemas/innerCore"));
}

#[test]
fn recursive_resolving() {
    let _g = common::setup(common::V3_MODULE);
    let mut parse_options = ParseOptions::new();
    parse_options.resolve_fully = true;
    let open_api = read("recursive.yaml", None, &parse_options).unwrap();
    let schema = media_schema(response(&path(&open_api, "/myPath").get_get().unwrap(), "200").get_content(), "application/json");
    assert!(schema.get_properties().unwrap().get("myProp").is_some());
    if json_string(&open_api).is_err() {
        panic!("Recursive loop found");
    }
}

#[test]
fn recursive_resolving2() {
    let _g = common::setup(common::V3_MODULE);
    let parse_options = opts(true, true);
    let open_api = read("recursive2.yaml", None, &parse_options);
    let open_api = open_api.unwrap();
    if json_string(&open_api).is_err() {
        panic!("Recursive loop found");
    }
}

#[test]
fn recursive_resolving_issue1751() {
    let _g = common::setup(common::V3_MODULE);
    let parse_options = opts(true, true);
    let open_api = read("issue_1751.yaml", None, &parse_options).expect("OpenAPI should be parsed successfully");
    assert!(schemas(&open_api).get("NestedObject").is_some(), "NestedObject schema should be present");
    // a successful writeValueAsString is never null
    if let Err(e) = json_string(&open_api) {
        panic!("Recursive loop found: {e:?}");
    }
}

#[test]
fn recursive_resolving_issue1751_mutual_recursion() {
    let _g = common::setup(common::V3_MODULE);
    let parse_options = opts(true, true);
    let open_api = read("issue_1751_mutual_recursion.yaml", None, &parse_options).expect("OpenAPI should be parsed successfully");
    assert!(open_api.get_components().is_some(), "Components should not be null");
    assert!(schemas(&open_api).get("SchemaA").is_some(), "SchemaA should be present");
    assert!(schemas(&open_api).get("SchemaB").is_some(), "SchemaB should be present");
    assert_eq!(schemas(&open_api).get("SchemaA").unwrap().get_type(), s("object"), "SchemaA should be object type");
    assert_eq!(schemas(&open_api).get("SchemaB").unwrap().get_type(), s("object"), "SchemaB should be object type");
    if let Err(e) = json_string(&open_api) {
        panic!("Recursive loop found: {e:?}");
    }
}

#[test]
fn recursive_resolving_issue1751_recursive_array_items() {
    let _g = common::setup(common::V3_MODULE);
    let parse_options = opts(true, true);
    let open_api = read("issue_1751_recursive_array.yaml", None, &parse_options).expect("OpenAPI should be parsed successfully");
    let tree_node = schemas(&open_api).get("TreeNode").expect("TreeNode schema should be present");
    assert_eq!(tree_node.get_type(), s("object"), "TreeNode should be object type");
    let value_property = tree_node.get_properties().unwrap().get("value").expect("TreeNode should have a value property");
    assert_eq!(value_property.get_type(), s("integer"), "value property should be integer type");
    if let Err(e) = json_string(&open_api) {
        panic!("Recursive loop found: {e:?}");
    }
}

#[test]
fn recursive_issue984() {
    let _g = common::setup(common::V3_MODULE);
    let parse_options = opts(true, true);
    let Some(open_api) = read("issue-984-simple.yaml", None, &parse_options) else { panic!("failed parsing issue-984") };
    // Json.pretty swallows exceptions (returns null)
    let _ = json_pretty(&open_api);
}

#[test]
fn property_name_mixup() {
    let _g = common::setup(common::V3_MODULE);
    let mut parse_options = ParseOptions::new();
    parse_options.resolve_fully = true;
    let open_api = read("simple.yaml", None, &parse_options).unwrap();
    let name = prop(&schemas(&open_api).get("Manufacturer").unwrap(), "name");
    assert!(name.is_kind(SchemaKind::String));
    assert_eq!(name.get_example(), Some(JavaObject::string("ACME Corporation")));
    let schema = media_schema(response(&path(&open_api, "/inventory").get_get().unwrap(), "200").get_content(), "application/json");
    assert!(schema.is_array_schema());
    let manufacturer = prop(&schema.get_items().unwrap(), "manufacturer");
    assert!(manufacturer.is_object_schema());
    assert_eq!(prop(&manufacturer, "name").get_example(), Some(JavaObject::string("ACME Corporation")));
}

#[test]
fn test_resolve_array_schema_items_null_pointer_exception() {
    let _g = common::setup(common::V3_MODULE);
    let mut options = ParseOptions::new();
    options.resolve = true;
    let actual_location = "C:/Users/ggregory/git/r/api-gateway/ais-swagger-test-fixtures/src/test/resources/APIs-guru/openapi-directory-master/APIs/clearblade.com/3.0/swagger.yaml";
    let output = read(actual_location, None, &options);
    OpenAPIResolver::with_location(output.as_ref(), None, Some(actual_location)).unwrap().resolve().unwrap();
}

/// `@Test(dataProvider = "explicitObjectSchemaProvider")`
#[test]
fn test_issue2113() {
    let _g = common::setup(common::V3_MODULE);
    for explicit_object_schema in [true, false] {
        let mut options = opts(true, true);
        options.explicit_object_schema = explicit_object_schema;

        let open_api = read_location("issue_2113.yaml", Some(AUTHS), &options).unwrap();
        let schema = media_schema(path(&open_api, "/foo").get_post().unwrap().get_request_body().unwrap().get_content(), "application/json");
        assert!(schema.is_object_schema());
        if explicit_object_schema {
            assert_eq!(prop(&schema, "goo").get_type(), s("object"));
        } else {
            assert!(prop(&schema, "goo").get_type().is_none());
        }
    }
}
