//! Port of `io.swagger.v3.parser.util.OpenAPIDeserializerTest` (swagger-parser-v3).

mod common;

use jackson::yaml::YamlReadFeatures;
use jackson::{BigDecimal, JavaObject, JsonNode};
use std::collections::HashSet;
use std::sync::Arc;
use swagger_parser::core::{AuthorizationValue, ParseOptions, SwaggerParseResult};
use swagger_parser::java::JList;
use swagger_parser::models::*;
use swagger_parser::parser::{OpenAPIDeserializer, OpenAPIResolver, OpenAPIV3Parser};

/// `List<AuthorizationValue> auths = new ArrayList<>()`
fn auths() -> Vec<AuthorizationValue> {
    Vec::new()
}

/// `new OpenAPIV3Parser().readContents(s, null, null)`
fn read_contents(s: &str) -> SwaggerParseResult {
    OpenAPIV3Parser::new().read_contents(s, None, None)
}

/// `new OpenAPIV3Parser().readContents(s, auths, options)` with `options.setResolve(true)`
fn read_contents_resolve(s: &str) -> SwaggerParseResult {
    let options = ParseOptions { resolve: true, ..Default::default() };
    OpenAPIV3Parser::new().read_contents(s, Some(&auths()), Some(&options))
}

/// `new ParseOptions()` with resolve, resolveFully and resolveCombinators set
fn resolve_fully_options() -> ParseOptions {
    ParseOptions { resolve: true, resolve_fully: true, resolve_combinators: true, ..Default::default() }
}

/// `result.getMessages()` (null would throw on use in Java)
fn messages(r: &SwaggerParseResult) -> Vec<String> {
    r.messages.clone().expect("messages")
}

/// `new HashSet<>(result.getMessages())`
fn message_set(r: &SwaggerParseResult) -> HashSet<String> {
    messages(r).into_iter().collect()
}

/// `new OpenAPIResolver(openAPI, null).resolve()`
fn resolve(o: Option<OpenAPI>) -> OpenAPI {
    OpenAPIResolver::with_location(o.as_ref(), None, None).unwrap().resolve().unwrap().unwrap()
}

fn s(v: &str) -> JavaObject {
    JavaObject::string(v)
}

fn big(v: &str) -> BigDecimal {
    BigDecimal::parse(v).unwrap()
}

fn big_obj(v: &str) -> JavaObject {
    JavaObject::BigDecimal(Arc::new(big(v)))
}

/// `new ObjectMapper(new YAMLFactory()).readTree(Files.readAllBytes(<test resource>))`
fn read_yaml_resource(name: &str) -> JsonNode {
    let text = common::read_file(&format!("src/test/resources/{name}"));
    jackson::yaml::read_tree(&text, YamlReadFeatures::swagger_core()).unwrap()
}

/// `@DataProvider(name = "data")`
fn data() -> Vec<JsonNode> {
    vec![read_yaml_resource("oas3.yaml.template")]
}

/// `new OpenAPIDeserializer().deserialize(rootNode)`
fn deserialize(root: &JsonNode) -> SwaggerParseResult {
    OpenAPIDeserializer::new().deserialize(root)
}

fn map_get(o: &JavaObject, key: &str) -> JavaObject {
    match o {
        JavaObject::Map(m) => m.get(key).cloned().unwrap_or(JavaObject::Null),
        other => panic!("not a Map: {other:?}"),
    }
}

fn paths(o: &OpenAPI) -> Paths {
    o.get_paths().unwrap()
}

fn path(o: &OpenAPI, p: &str) -> PathItem {
    paths(o).get(p).unwrap()
}

fn schemas(o: &OpenAPI) -> swagger_parser::java::JMap<Schema> {
    o.get_components().unwrap().get_schemas().unwrap()
}

fn find_param(op: &Operation, name: &str) -> Parameter {
    op.get_parameters()
        .unwrap()
        .to_vec()
        .into_iter()
        .find(|p| p.get_name().as_deref() == Some(name))
        .unwrap_or_else(|| panic!("Can't find parameter={name}"))
}

/// `new Calendar.Builder().setDate(y, m, d).build().getTime()`
// JDK-specific: Calendar.Builder builds the date in the JVM default time zone, while DateSchema parses
// enum/default values as UTC dates; the Java test only passes on a UTC JVM. The expectation here is the
// UTC midnight of the date (what the Java assertion compares against on such a JVM).
fn date(y: i64, m0: u32, d: u32) -> JavaObject {
    let days = jackson::convert::days_from_civil(y, m0 + 1, d);
    JavaObject::Date(days * 86_400_000)
}

/// `OffsetDateTime.of(y, mo, d, h, mi, s, nanos, ZoneOffset.ofTotalSeconds(off))`
// The JDK factory's eight parameters, in its order, so the calls read as the Java test's.
#[allow(clippy::too_many_arguments)]
fn odt(y: i64, mo: u32, d: u32, h: i64, mi: i64, sec: i64, nano: u32, off: i32) -> JavaObject {
    let days = jackson::convert::days_from_civil(y, mo, d);
    let local = days * 86_400 + h * 3600 + mi * 60 + sec;
    JavaObject::OffsetDateTime(jackson::object::OffsetDateTime { epoch_second: local - off as i64, nano, offset_seconds: off })
}

#[test]
fn test_issue1072() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.0\n",
        "info:\n",
        "  title: Test\n",
        "  version: 1.0.0\n",
        "\n",
        "paths:\n",
        "  /value:\n",
        "    get:\n",
        "      operationId: getValues\n",
        "      responses:\n",
        "        200:\n",
        "          description: Successful response\n",
        "          content:\n",
        "            application/json:\n",
        "              schema:\n",
        "                $ref: \"#/components/schemas/ComponentA\"\n",
        "components:\n",
        "  schemas:\n",
        "    ComponentA:\n",
        "      description: Component A\n",
        "      type: object\n",
        "      allOf:\n",
        "        - type: object\n",
        "          properties:\n",
        "            attributeWithoutType:\n",
        "              allOf:\n",
        "              - $ref: \"#/components/schemas/ComponentB\"\n",
        "              default: \"coucou\"\n",
        "            attributeWithWrongType:\n",
        "              type: object\n",
        "              allOf:\n",
        "                - $ref: \"#/components/schemas/ComponentB\"\n",
        "              default: \"coucou\"\n",
        "            correctAttribute:\n",
        "              type: string\n",
        "              allOf:\n",
        "                - $ref: \"#/components/schemas/ComponentB\"\n",
        "              default: \"coucou\"\n",
        "    ComponentB:\n",
        "      description: Component B\n",
        "      type: string"
    );
    let result = OpenAPIV3Parser::new().read_contents(yaml, None, None);
    assert!(result.openapi.is_some());
}

#[test]
fn test_empty_definitions() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.0\n",
        "servers:\n",
        "  - url: 'http://abc:5555/mypath'\n",
        "info:\n",
        "  version: '1.0'\n",
        "  title: dd\n",
        "paths:\n",
        "  /resource1/Id:\n",
        "    post:\n",
        "      description: ''\n",
        "      operationId: postOp\n",
        "      responses:\n",
        "        '200':\n",
        "          description: Successful\n",
        "        '401':\n",
        "          description: Access Denied\n",
        "      requestBody:\n",
        "        content:\n",
        "          application/json:\n",
        "            schema:\n",
        "              $ref: \"#/components/schemas/mydefinition\"\n",
        "        required: true\n",
        "components:\n",
        "  schemas:\n",
        "    mydefinition: {}"
    );
    let result = read_contents(yaml);
    let open_api = result.openapi.clone().unwrap();
    assert!(schemas(&open_api).get("mydefinition").is_some());
}

#[test]
fn test_identifier_and_url_invalid() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.1.0\n",
        "servers:\n",
        "  - url: 'http://abc:5555/mypath'\n",
        "info:\n",
        "  version: '1.0'\n",
        "  title: dd\n",
        "  license:\n",
        "    name: test\n",
        "    url: http://example.com\n",
        "    identifier: test\n",
        "paths:\n",
        "  /resource1/Id:\n",
        "    post:\n",
        "      description: ''\n",
        "      operationId: postOp\n",
        "      responses:\n",
        "        '200':\n",
        "          description: Successful\n",
        "        '401':\n",
        "          description: Access Denied\n",
        "      requestBody:\n",
        "        content:\n",
        "          application/json:\n",
        "            schema:\n",
        "              $ref: \"#/components/schemas/mydefinition\"\n",
        "        required: true\n",
        "components:\n",
        "  schemas:\n",
        "    mydefinition: {}"
    );
    let result = read_contents(yaml);
    assert!(result.openapi.is_some());
    assert_eq!(messages(&result).len(), 1);
    assert!(messages(&result)[0].contains("identifier"));
}

#[test]
fn test_url_valid() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.1.0\n",
        "servers:\n",
        "  - url: 'http://abc:5555/mypath'\n",
        "info:\n",
        "  version: '1.0'\n",
        "  title: dd\n",
        "  license:\n",
        "    name: test\n",
        "    url: http://example.com\n",
        "paths:\n",
        "  /resource1/Id:\n",
        "    post:\n",
        "      description: ''\n",
        "      operationId: postOp\n",
        "      responses:\n",
        "        '200':\n",
        "          description: Successful\n",
        "        '401':\n",
        "          description: Access Denied\n",
        "      requestBody:\n",
        "        content:\n",
        "          application/json:\n",
        "            schema:\n",
        "              $ref: \"#/components/schemas/mydefinition\"\n",
        "        required: true\n",
        "components:\n",
        "  schemas:\n",
        "    mydefinition: {}"
    );
    let result = read_contents(yaml);
    assert!(result.openapi.is_some());
    assert_eq!(messages(&result).len(), 0);
}

#[test]
fn test_identifier_valid() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.1.0\n",
        "servers:\n",
        "  - url: 'http://abc:5555/mypath'\n",
        "info:\n",
        "  version: '1.0'\n",
        "  title: dd\n",
        "  license:\n",
        "    name: test\n",
        "    identifier: abc\n",
        "paths:\n",
        "  /resource1/Id:\n",
        "    post:\n",
        "      description: ''\n",
        "      operationId: postOp\n",
        "      responses:\n",
        "        '200':\n",
        "          description: Successful\n",
        "        '401':\n",
        "          description: Access Denied\n",
        "      requestBody:\n",
        "        content:\n",
        "          application/json:\n",
        "            schema:\n",
        "              $ref: \"#/components/schemas/mydefinition\"\n",
        "        required: true\n",
        "components:\n",
        "  schemas:\n",
        "    mydefinition: {}"
    );
    let result = read_contents(yaml);
    assert!(result.openapi.is_some());
    assert_eq!(messages(&result).len(), 0);
}

#[test]
fn test_security_deserialization() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.0\n",
        "security:\n",
        "  - api_key1: []\n",
        "    api_key2: []\n",
        "  - api_key3: []\n"
    );
    let result = read_contents(yaml);
    let open_api = result.openapi.clone().unwrap();
    let security = open_api.get_security().unwrap();
    assert!(security.len() == 2);
}

#[test]
fn test_schema() {
    let _g = common::setup(common::V3_MODULE);
    let json = concat!(
        "{\n",
        "  \"type\":\"object\",\n",
        "  \"properties\": {\n",
        "    \"data\": {\n",
        "      \"properties\": {\n",
        "        \"name\": {\n",
        "          \"type\": \"string\",\n",
        "          \"minLength\": 1\n",
        "        }\n",
        "      }\n",
        "    }\n",
        "  },\n",
        "  \"required\": [\n",
        "    \"data\"\n",
        "  ]\n",
        "}"
    );
    // Json.mapper().readValue(json, Schema.class)
    let node = jackson::json::read_tree(json, Default::default()).unwrap();
    let m = Schema::from_tree(&node, Mapper::Json30).unwrap();
    let properties = m.get_properties().unwrap();
    assert!(properties.keys().len() == 1);
    let data = properties.get("data").unwrap();
    assert!(data.kind() == SchemaKind::Object);
    let inner_properties = data.get_properties().unwrap();
    assert!(inner_properties.keys().len() == 1);
    let name = inner_properties.get("name").unwrap();
    assert!(name.kind() == SchemaKind::String);
}

#[test]
fn test_array_schema() {
    let _g = common::setup(common::V3_MODULE);
    let json = concat!(
        "{\n",
        "  \"properties\": {\n",
        "    \"data\": {\n",
        "      \"description\": \"the array type\",\n",
        "      \"type\": \"array\",\n",
        "      \"items\": {\n",
        "        \"properties\": {\n",
        "          \"name\": {\n",
        "            \"description\": \"the inner type\",\n",
        "            \"type\": \"string\",\n",
        "            \"minLength\": 1\n",
        "          }\n",
        "        }\n",
        "      }\n",
        "    }\n",
        "  },\n",
        "  \"required\": [\n",
        "    \"data\"\n",
        "  ]\n",
        "}"
    );
    let node = jackson::json::read_tree(json, Default::default()).unwrap();
    let m = Schema::from_tree(&node, Mapper::Json30).unwrap();
    let data = m.get_properties().unwrap().get("data").unwrap();
    assert!(data.kind() == SchemaKind::Array);
    assert_eq!(data.get_description().as_deref(), Some("the array type"));
    let inner = data.get_items();
    assert!(inner.is_some());
    let inner = inner.unwrap();
    assert!(inner.kind() == SchemaKind::Object);
    let name = inner.get_properties().unwrap().get("name").unwrap();
    assert_eq!(name.get_description().as_deref(), Some("the inner type"));
    assert!(name.get_min_length().unwrap() == 1);
}

#[test]
fn test_empty() {
    let _g = common::setup(common::V3_MODULE);
    let json = "{}";
    let result = read_contents(json);
    assert!(message_set(&result).contains("attribute openapi is missing"));
}

#[test]
fn test_security() {
    let _g = common::setup(common::V3_MODULE);
    let json = concat!(
        "{\n",
        "  \"openapi\": \"3.0.0\",\n",
        "  \"security\": [\n",
        "    {\n",
        "      \"petstore_auth\": [\n",
        "        \"write:pets\",\n",
        "        \"read:pets\"\n",
        "      ]\n",
        "    }\n",
        "  ]\n",
        "}"
    );
    let result = read_contents(json);
    let open_api = result.openapi.clone().unwrap();
    assert!(open_api.get_security().is_some());
    let security = open_api.get_security().unwrap();
    assert!(security.len() == 1);
    assert!(security.get(0).unwrap().len() == 1);
    let requirement = security.get(0).unwrap().get("petstore_auth").unwrap();
    assert!(requirement.len() == 2);
    let requirements: HashSet<String> = requirement.to_vec().into_iter().collect();
    assert!(requirements.contains("read:pets"));
    assert!(requirements.contains("write:pets"));
}

#[test]
fn test_security_definition() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.0\n",
        "servers: []\n",
        "paths:\n",
        "  /pet:\n",
        "    get:\n",
        "      security:\n",
        "        - basic_auth: []\n",
        "          api_key: []\n",
        "      responses:\n",
        "        default:\n",
        "          description: Default response\n",
        "info:\n",
        "  version: ''\n",
        "  title: ''\n",
        "components:\n",
        "  securitySchemes:\n",
        "    basic_auth:\n",
        "      type: http\n",
        "      x-foo: basicBar\n",
        "      scheme: basic\n",
        "    api_key:\n",
        "      type: apiKey\n",
        "      name: api_key\n",
        "      in: header\n",
        "      description: api key description\n",
        "      x-foo: apiKeyBar"
    );
    let result = read_contents(yaml);
    let open_api = result.openapi.clone().unwrap();
    let schemes = open_api.get_components().unwrap().get_security_schemes();
    assert!(schemes.is_some());
    let schemes = schemes.unwrap();
    assert!(schemes.keys().len() == 2);

    // Basic Authentication
    let definition_basic = schemes.get("basic_auth");
    assert!(definition_basic.is_some());
    let definition_basic = definition_basic.unwrap();
    assert_eq!(definition_basic.get_type(), Some(SecuritySchemeType::Http));
    assert_eq!(definition_basic.get_extensions().unwrap().get("x-foo"), Some(s("basicBar")));
    // API Key Authentication
    let definition = schemes.get("api_key");
    assert!(definition.is_some());
    let definition = definition.unwrap();
    assert_eq!(definition.get_type(), Some(SecuritySchemeType::ApiKey));

    let api_key = definition;
    assert_eq!(api_key.get_name().as_deref(), Some("api_key"));
    assert_eq!(api_key.get_in(), Some(SecuritySchemeIn::Header));
    assert_eq!(api_key.get_description().as_deref(), Some("api key description"));
    assert_eq!(api_key.get_extensions().unwrap().get("x-foo"), Some(s("apiKeyBar")));
}

#[test]
fn test_security_definition_with_missing_attribute() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.0\n",
        "components:\n",
        "  securitySchemes:\n",
        "    api_key:\n",
        "      description: api key description"
    );
    let result = read_contents(yaml);
    assert!(message_set(&result).contains("attribute components.securitySchemes.api_key.type is missing"));
}

#[test]
fn test_security_definition_api_key_with_missing_attribute_in() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.0\n",
        "components:\n",
        "  securitySchemes:\n",
        "    api_key:\n",
        "      type: apiKey\n",
        "      name: X-API-KEY"
    );
    let result = read_contents(yaml);
    assert!(message_set(&result).contains("attribute components.securitySchemes.api_key.in is missing"));
}

#[test]
fn test_security_definition_api_key_with_invalid_attribute_in() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.0\n",
        "components:\n",
        "  securitySchemes:\n",
        "    api_key:\n",
        "      type: apiKey\n",
        "      name: X-API-KEY\n",
        "      in: cukie"
    );
    let result = read_contents(yaml);
    assert!(message_set(&result).contains("attribute components.securitySchemes.api_key.in is not of type `cookie|header|query`"));
}

#[test]
fn test_security_definition_api_key_valid() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.0\n",
        "components:\n",
        "  securitySchemes:\n",
        "    api_key:\n",
        "      type: apiKey\n",
        "      name: X-API-KEY\n",
        "      in: cookie"
    );
    let result = read_contents(yaml);
    assert!(!message_set(&result).contains("attribute components.securitySchemes.api_key.in is not of type `cookie|header|query`"));
}

#[test]
fn test_root_info() {
    let _g = common::setup(common::V3_MODULE);
    let json = concat!(
        "{\n",
        "\t\"openapi\": \"3.0.0\",\n",
        "\t\"foo\": \"bar\",\n",
        "\t\"info\": \"invalid\"\n",
        "}"
    );
    let result = read_contents(json);
    let messages = message_set(&result);
    assert!(messages.contains("attribute foo is unexpected"));
    assert!(messages.contains("attribute info is not of type `object`"));
}

#[test]
fn test_contact() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.0\n",
        "servers: []\n",
        "info:\n",
        "  title: title\n",
        "  bad: bad\n",
        "  x-foo: bar\n",
        "  description: description\n",
        "  termsOfService: tos\n",
        "  contact:\n",
        "    name: tony\n",
        "    url: url\n",
        "    email: email\n",
        "    invalid: invalid\n",
        "    x-fun: true\n",
        "  version: version\n",
        "paths: {}"
    );
    let result = read_contents(yaml);
    let messages = message_set(&result);
    let info = result.openapi.clone().unwrap().get_info().unwrap();
    assert_eq!(info.get_title().as_deref(), Some("title"));
    assert_eq!(info.get_description().as_deref(), Some("description"));
    assert_eq!(info.get_terms_of_service().as_deref(), Some("tos"));
    assert_eq!(info.get_version().as_deref(), Some("version"));

    let contact = info.get_contact().unwrap();
    assert_eq!(contact.get_name().as_deref(), Some("tony"));
    assert_eq!(contact.get_url().as_deref(), Some("url"));
    assert_eq!(contact.get_email().as_deref(), Some("email"));

    assert!(messages.contains("attribute info.bad is unexpected"));
    assert!(messages.contains("attribute info.contact.invalid is unexpected"));

    assert_eq!(info.get_extensions().unwrap().get("x-foo").unwrap().to_java_string(), "bar");
}

#[test]
fn test_responses() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.0\n",
        "servers: []\n",
        "info:\n",
        "  version: ''\n",
        "  title: ''\n",
        "paths: {}\n",
        "components:\n",
        "  responses:\n",
        "    foo:\n",
        "      description: description\n",
        "      bar: baz\n",
        "      x-foo: bar"
    );
    let result = read_contents(yaml);
    assert!(message_set(&result).contains("attribute components.responses.foo.bar is unexpected"));
    let response = result.openapi.clone().unwrap().get_components().unwrap().get_responses().unwrap().get("foo").unwrap();
    assert_eq!(response.get_extensions().unwrap().get("x-foo").unwrap().to_java_string(), "bar");
}

#[test]
fn test_license() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.0\n",
        "servers: []\n",
        "info:\n",
        "  license:\n",
        "    invalid: true\n",
        "    x-valid:\n",
        "      isValid: true\n",
        "  version: ''\n"
    );
    let result = read_contents(yaml);
    let messages = message_set(&result);
    assert!(messages.contains("attribute info.license.invalid is unexpected"));
    assert!(messages.contains("attribute info.title is missing"));
    assert!(messages.contains("attribute paths is missing"));

    let license = result.openapi.clone().unwrap().get_info().unwrap().get_license().unwrap();
    assert_eq!(map_get(&license.get_extensions().unwrap().get("x-valid").unwrap(), "isValid"), JavaObject::Boolean(true));
}

#[test]
fn test_definitions() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.0\n",
        "servers: []\n",
        "info:\n",
        "  version: ''\n",
        "  title: ''\n",
        "paths: {}\n",
        "components:\n",
        "  schemas:\n",
        "    invalid: true\n",
        "    Person:\n",
        "      required:\n",
        "        - id\n",
        "        - name\n",
        "      properties:\n",
        "        id:\n",
        "          type: integer\n",
        "          format: int64\n",
        "        name:\n",
        "          type: string"
    );
    let result = read_contents(yaml);
    assert!(message_set(&result).contains("attribute components.schemas.invalid is not of type `object`"));
    let open_api = result.openapi.clone().unwrap();
    // instanceof Schema: a non-null schema
    assert!(schemas(&open_api).get("Person").is_some());

    let required = schemas(&open_api).get("Person").unwrap().get_required().unwrap();
    let required_keys: HashSet<String> = required.to_vec().into_iter().collect();
    assert!(required_keys.contains("id"));
    assert!(required_keys.contains("name"));
    assert!(required_keys.len() == 2);
}

#[test]
fn test_nested_definitions() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.0\n",
        "servers: []\n",
        "info:\n",
        "  version: ''\n",
        "  title: ''\n",
        "paths: {}\n",
        "components:\n",
        "  schemas:\n",
        "    Person:\n",
        "      required:\n",
        "        - id\n",
        "        - name\n",
        "      properties:\n",
        "        id:\n",
        "          type: integer\n",
        "          format: int64\n",
        "        name:\n",
        "          type: string\n",
        "        address:\n",
        "          $ref: \"#/components/schemas/Address\"\n",
        "    Address:\n",
        "      required:\n",
        "        - zip\n",
        "      properties:\n",
        "        street:\n",
        "          type: string\n",
        "        zip:\n",
        "          type: integer\n",
        "          format: int32\n",
        "          minimum: 0\n",
        "          exclusiveMinimum: true\n",
        "          maximum: 99999\n",
        "          exclusiveMaximum: true"
    );
    let result = read_contents(yaml);
    let open_api = result.openapi.clone().unwrap();
    assert!(schemas(&open_api).get("Person").is_some());
    assert!(schemas(&open_api).get("Address").is_some());

    let person = schemas(&open_api).get("Person").unwrap();
    let property = person.get_properties().unwrap().get("address").unwrap();
    assert!(property.get_ref().is_some());

    let zip = schemas(&open_api).get("Address").unwrap().get_properties().unwrap().get("zip").unwrap();
    assert!(zip.kind() == SchemaKind::Integer);

    assert_eq!(zip.get_minimum(), Some(big("0")));
    assert!(zip.get_exclusive_minimum().unwrap());

    assert_eq!(zip.get_maximum(), Some(big("99999")));
    assert!(zip.get_exclusive_maximum().unwrap());
}

#[test]
fn test_paths() {
    let _g = common::setup(common::V3_MODULE);
    let json = concat!(
        "{\n",
        "  \"openapi\": \"3.0.0\",\n",
        "  \"paths\": {\n",
        "    \"/pet\": {\n",
        "      \"foo\": \"bar\",\n",
        "      \"get\": {\n",
        "        \"security\": [\n",
        "          {\n",
        "            \"petstore_auth\": [\n",
        "              \"write:pets\",\n",
        "              \"read:pets\"\n",
        "            ]\n",
        "          }\n",
        "        ]\n",
        "      }\n",
        "    }\n",
        "  }\n",
        "}"
    );
    let result = read_contents(json);
    assert!(message_set(&result).contains("attribute paths.'/pet'.foo is unexpected"));
    let open_api = result.openapi.clone().unwrap();

    let path = paths(&open_api).get("/pet");
    assert!(path.is_some());
    let operation = path.unwrap().get_get();
    assert!(operation.is_some());
    let security = operation.unwrap().get_security().unwrap();

    assert!(security.len() == 1);
    let requirement = security.get(0).unwrap();

    assert!(requirement.contains_key("petstore_auth"));
    let scopes_list = requirement.get("petstore_auth").unwrap();

    let scopes: HashSet<String> = scopes_list.to_vec().into_iter().collect();
    assert!(scopes.contains("read:pets"));
    assert!(scopes.contains("write:pets"));
}

#[test]
fn test_paths_with_ref_response() {
    let _g = common::setup(common::V3_MODULE);
    let json = concat!(
        "{\n",
        "  \"openapi\": \"3.0.0\",\n",
        "  \"paths\": {\n",
        "    \"/pet\": {\n",
        "      \"get\": {\n",
        "        \"responses\": {\n",
        "          \"200\": {\n",
        "            \"$ref\": \"#/components/responses/OK\"",
        "          }\n",
        "        }\n",
        "      }\n",
        "    }\n",
        "  }\n",
        "}"
    );
    let result = read_contents(json);
    let open_api = result.openapi.clone().unwrap();

    let path = paths(&open_api).get("/pet");
    assert!(path.is_some());
    let operation = path.unwrap().get_get();
    assert!(operation.is_some());
    let responses = operation.unwrap().get_responses().unwrap();
    assert!(responses.contains_key("200"));
    // assertEquals(ApiResponse.class, getClass()): ApiResponse has no subclasses; the value is an ApiResponse
    let ref_response: ApiResponse = responses.get("200").unwrap();
    assert_eq!(ref_response.get_ref().as_deref(), Some("#/components/responses/OK"));
}

#[test]
fn test_array_model_definition() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.0\n",
        "paths:\n",
        "  \"/store/inventory\":\n",
        "    get:\n",
        "      responses:\n",
        "        '200':\n",
        "          description: successful operation\n",
        "          content:\n",
        "            application/json:\n",
        "              schema:\n",
        "                type: object\n",
        "                additionalProperties:\n",
        "                  type: integer\n",
        "                  format: int32\n"
    );
    let result = read_contents(yaml);
    let open_api = result.openapi.clone().unwrap();
    let response = path(&open_api, "/store/inventory").get_get().unwrap().get_responses().unwrap().get("200").unwrap().get_content().unwrap().get("application/json").unwrap().get_schema().unwrap();
    assert!(response.get_additional_properties().is_some());
}

#[test]
fn test_additional_properties_boolean() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.0\n",
        "info:\n",
        "  title: Test\n",
        "  version: 1.0.0\n",
        "paths:\n",
        "  \"/store/inventory\":\n",
        "    post:\n",
        "      requestBody:\n",
        "        content:\n",
        "          application/json:\n",
        "            schema:\n",
        "              additionalProperties: true\n",
        "      responses:\n",
        "        '200':\n",
        "          description: successful operation\n",
        "          content:\n",
        "            application/json:\n",
        "              schema:\n",
        "                additionalProperties: false\n"
    );
    let result = read_contents(yaml);
    assert_eq!(messages(&result), Vec::<String>::new());

    let open_api = result.openapi.clone().unwrap();
    let post = path(&open_api, "/store/inventory").get_post().unwrap();
    let body = post.get_request_body().unwrap().get_content().unwrap().get("application/json").unwrap().get_schema().unwrap();
    assert_eq!(body.get_additional_properties(), Some(AdditionalProperties::Bool(true)));
    assert_eq!(body.kind(), SchemaKind::Map);

    let response = post.get_responses().unwrap().get("200").unwrap().get_content().unwrap().get("application/json").unwrap().get_schema().unwrap();
    assert_eq!(response.get_additional_properties(), Some(AdditionalProperties::Bool(false)));
    assert_eq!(response.kind(), SchemaKind::Object);
}

#[test]
fn test_array_query_param() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.0\n",
        "servers: []\n",
        "paths:\n",
        "  /pet/findByStatus:\n",
        "    get:\n",
        "      parameters:\n",
        "        - name: status\n",
        "          in: query\n",
        "          description: Status values that need to be considered for filter\n",
        "          required: false\n",
        "          style: pipeDelimited\n",
        "          schema:\n",
        "            type: array\n",
        "            items:\n",
        "              type: string\n",
        "            default: available\n",
        "      responses:\n",
        "        '200':\n",
        "          description: successful operation\n",
        "          content:\n",
        "            '*/*':\n",
        "              schema:\n",
        "                $ref: #/components/schemas/PetArray\n",
        "info:\n",
        "  version: ''\n",
        "  title: ''"
    );
    let result = read_contents(yaml);
    let open_api = result.openapi.clone().unwrap();
    let param = path(&open_api, "/pet/findByStatus").get_get().unwrap().get_parameters().unwrap().get(0).unwrap();

    assert!(param.kind() == ParameterKind::Query);
    let p = param.get_schema().unwrap();

    assert_eq!(p.get_type().as_deref(), Some("array"));
    assert!(p.kind() == SchemaKind::Array);
    assert!(p.get_items().unwrap().kind() == SchemaKind::String);
}

#[test]
fn test_array_items() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.0\n",
        "info:\n",
        "  title: Test\n",
        "  version: 1.0.0\n",
        "paths:\n",
        "  \"/store/inventory\":\n",
        "    post:\n",
        "      requestBody:\n",
        "        content:\n",
        "          application/json:\n",
        "            schema:\n",
        "              type: array\n",
        "              minItems: 1\n",
        "      responses:\n",
        "        '200':\n",
        "          description: successful operation\n",
        "          content:\n",
        "            application/json:\n",
        "              schema:\n",
        "                items:\n",
        "                  type: string"
    );
    let result = read_contents(yaml);
    assert_eq!(messages(&result), vec!["attribute paths.'/store/inventory'(post).requestBody.content.'application/json'.schema.items is missing".to_string()]);

    let open_api = result.openapi.clone().unwrap();
    let post = path(&open_api, "/store/inventory").get_post().unwrap();
    let body = post.get_request_body().unwrap().get_content().unwrap().get("application/json").unwrap().get_schema().unwrap();
    assert!(body.kind() != SchemaKind::Array, "body is an ArraySchema");
    assert_eq!(body.get_type().as_deref(), Some("array"));
    assert_eq!(body.get_min_items(), Some(1));

    let response = post.get_responses().unwrap().get("200").unwrap().get_content().unwrap().get("application/json").unwrap().get_schema().unwrap();
    assert!(response.kind() == SchemaKind::Array, "response is an ArraySchema");
    assert_eq!(body.get_type().as_deref(), Some("array"));
}

/// it should read a top-level extension per https://github.com/openAPI-api/validator-badge/issues/59
#[test]
fn test_toplevel_extension() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.0\n",
        "servers: []\n",
        "x-foo: woof\n",
        "info:\n",
        "  version: 0.0.0\n",
        "  title: Simple API\n",
        "paths:\n",
        "  /:\n",
        "    get:\n",
        "      responses:\n",
        "        '200':\n",
        "          description: OK"
    );
    let result = read_contents(yaml);
    assert!(result.openapi.clone().unwrap().get_extensions().is_some());
}

#[test]
fn test_deserialize_binary_string() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.0\n",
        "servers: []\n",
        "info:\n",
        "  title: foo\n",
        "  version: ''\n",
        "paths:\n",
        "  /test:\n",
        "    post:\n",
        "      responses:\n",
        "        '200':\n",
        "          description: ok\n",
        "      requestBody:\n",
        "        content:\n",
        "          application/json:\n",
        "            schema:\n",
        "              type: string\n",
        "              format: binary"
    );
    let result = read_contents(yaml);
    let resolved = resolve(result.openapi.clone());
    let schema = path(&resolved, "/test").get_post().unwrap().get_request_body().unwrap().get_content().unwrap().get("application/json").unwrap().get_schema().unwrap();
    assert!(schema.kind() == SchemaKind::Binary);
}

#[test]
fn test_deserialize_enum() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.0\n",
        "servers: []\n",
        "info:\n",
        "  version: 0.0.0\n",
        "  title: your title\n",
        "paths:\n",
        "  /persons:\n",
        "    get:\n",
        "      description: a test\n",
        "      responses:\n",
        "        '200':\n",
        "          description: Successful response\n",
        "          content:\n",
        "            '*/*':\n",
        "              schema:\n",
        "                type: object\n",
        "                properties:\n",
        "                  se:\n",
        "                    $ref: \"#/components/schemas/StringEnum\"\n",
        "                  ie:\n",
        "                    $ref: \"#/components/schemas/IntegerEnum\"\n",
        "                  ne:\n",
        "                    $ref: \"#/components/schemas/NumberEnum\"\n",
        "                  be:\n",
        "                    $ref: \"#/components/schemas/BooleanEnum\"\n",
        "                  ae:\n",
        "                    $ref: \"#/components/schemas/ArrayEnum\"\n",
        "                  oe:\n",
        "                    $ref: \"#/components/schemas/ObjectEnum\"\n",
        "components:\n",
        "  schemas:\n",
        "    StringEnum:\n",
        "      type: string\n",
        "      default: foo\n",
        "      enum:\n",
        "        - First\n",
        "        - Second\n",
        "    BooleanEnum:\n",
        "      enum:\n",
        "        - true \n",
        "        - false \n",
        "    IntegerEnum:\n",
        "      type: integer\n",
        "      default: 1\n",
        "      enum:\n",
        "        - -1\n",
        "        - 0\n",
        "        - 1\n",
        "    NumberEnum:\n",
        "      type: number\n",
        "      default: 3.14\n",
        "      enum:\n",
        "        - -1.151\n",
        "        - 0\n",
        "        - 1.6161\n",
        "        - 3.14\n",
        "    ArrayEnum:\n",
        "      type: array\n",
        "      items:\n",
        "        type: string\n",
        "      enum:\n",
        "        - - Camry\n",
        "          - Prius\n",
        "        - null\n",
        "        - - Pilot\n",
        "          - Passport\n",
        "        - - Rogue\n",
        "          - Leaf\n",
        "    ObjectEnum:\n",
        "      type: object\n",
        "      enum:\n",
        "        - make: Toyota\n",
        "          model: Prius\n",
        "        - make: Honda\n",
        "          model: Pilot\n",
        "        - make: Nissan\n",
        "          model: Leaf\n",
        "        - null\n"
    );
    let result = read_contents(yaml);
    let resolved = resolve(result.openapi.clone());

    let string_model = schemas(&resolved).get("StringEnum");
    assert!(string_model.is_some());
    let string_values = string_model.unwrap().get_enum().unwrap();
    assert_eq!(2, string_values.len());
    assert_eq!(Some(s("First")), string_values.get(0));
    assert_eq!(Some(s("Second")), string_values.get(1));

    let integer_model = schemas(&resolved).get("IntegerEnum");
    assert!(integer_model.is_some());
    let integer_impl = integer_model.unwrap();
    let integer_values = integer_impl.get_enum().unwrap();
    assert_eq!(3, integer_values.len());
    assert_eq!(Some(JavaObject::Integer(-1)), integer_values.get(0));
    assert_eq!(Some(JavaObject::Integer(0)), integer_values.get(1));
    assert_eq!(Some(JavaObject::Integer(1)), integer_values.get(2));
    assert_eq!(integer_impl.get_default(), Some(JavaObject::Integer(1)));

    let number_model = schemas(&resolved).get("NumberEnum");
    assert!(number_model.is_some());
    let number_impl = number_model.unwrap();
    let number_values = number_impl.get_enum().unwrap();
    assert_eq!(4, number_values.len());
    assert_eq!(Some(big_obj("-1.151")), number_values.get(0));
    assert_eq!(Some(big_obj("0")), number_values.get(1));
    assert_eq!(Some(big_obj("1.6161")), number_values.get(2));
    assert_eq!(Some(big_obj("3.14")), number_values.get(3));
    assert_eq!(number_impl.get_default(), Some(big_obj("3.14")));

    let boolean_model = schemas(&resolved).get("BooleanEnum").unwrap();
    assert_eq!(Some("boolean"), boolean_model.get_type().as_deref());
    let boolean_values = boolean_model.get_enum().unwrap();
    assert_eq!(2, boolean_values.len());
    assert_eq!(Some(JavaObject::Boolean(true)), boolean_values.get(0));
    assert_eq!(Some(JavaObject::Boolean(false)), boolean_values.get(1));

    let text_array = |a: &str, b: &str| JavaObject::Node(JsonNode::array(vec![JsonNode::text(a), JsonNode::text(b)]));
    let array_model = schemas(&resolved).get("ArrayEnum").unwrap();
    assert_eq!(Some("array"), array_model.get_type().as_deref());
    let array_values = array_model.get_enum().unwrap();
    assert_eq!(array_values.len(), 4);
    assert_eq!(array_values.get(0), Some(text_array("Camry", "Prius")));
    assert!(array_values.get(1).unwrap().is_null());
    assert_eq!(array_values.get(2), Some(text_array("Pilot", "Passport")));
    assert_eq!(array_values.get(3), Some(text_array("Rogue", "Leaf")));

    let object_node = |make: &str, model: &str| {
        let mut o = jackson::ObjectNode::new();
        o.insert("make".into(), JsonNode::text(make));
        o.insert("model".into(), JsonNode::text(model));
        JavaObject::Node(JsonNode::object(o))
    };
    let object_model = schemas(&resolved).get("ObjectEnum").unwrap();
    assert_eq!(Some("object"), object_model.get_type().as_deref());
    let object_values = object_model.get_enum().unwrap();
    assert_eq!(object_values.len(), 4);
    assert_eq!(object_values.get(0), Some(object_node("Toyota", "Prius")));
    assert_eq!(object_values.get(1), Some(object_node("Honda", "Pilot")));
    assert_eq!(object_values.get(2), Some(object_node("Nissan", "Leaf")));
    assert!(object_values.get(3).unwrap().is_null());
}

#[test]
fn test_enum_type() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { resolve: true, ..Default::default() };
    let result = OpenAPIV3Parser::new().read_location("./src/test/resources/issue-1090.yaml", None, Some(&options));
    assert!(result.openapi.is_some());
    let open_api = result.openapi.clone().unwrap();

    let some_obj = schemas(&open_api).get("SomeObj");
    assert!(some_obj.is_some());

    let properties = some_obj.unwrap().get_properties();
    assert!(properties.is_some());
    let properties = properties.unwrap();

    let iprop = properties.get("iprop").unwrap();
    assert_eq!(iprop.get_type().as_deref(), Some("integer"));
    assert_eq!(iprop.get_format().as_deref(), Some("int32"));

    let lprop = properties.get("lprop").unwrap();
    assert_eq!(lprop.get_type().as_deref(), Some("integer"));
    assert_eq!(lprop.get_format().as_deref(), Some("int64"));

    let nprop = properties.get("nprop").unwrap();
    assert_eq!(nprop.get_type().as_deref(), Some("number"));
}

#[test]
fn test_deserialize_date_string() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.0\n",
        "servers: []\n",
        "info:\n",
        "  version: 0.0.0\n",
        "  title: My Title\n",
        "paths:\n",
        "  /persons:\n",
        "    get:\n",
        "      description: a test\n",
        "      responses:\n",
        "        '200':\n",
        "          description: Successful response\n",
        "          content:\n",
        "            '*/*':\n",
        "              schema:\n",
        "                type: object\n",
        "                properties:\n",
        "                  date:\n",
        "                    $ref: \"#/components/schemas/DateString\"\n",
        "components:\n",
        "  schemas:\n",
        "    DateString:\n",
        "      type: string\n",
        "      format: date\n",
        "      default: 2019-01-01\n",
        "      enum:\n",
        "        - 2019-01-01\n",
        "        - Nope\n",
        "        - 2018-02-02\n",
        "        - 2017-03-03\n",
        "        - null\n",
        ""
    );
    let result = read_contents(yaml);
    let resolved = resolve(result.openapi.clone());

    let date_model = schemas(&resolved).get("DateString").unwrap();
    assert!(date_model.kind() == SchemaKind::Date);
    let date_values = date_model.get_enum().unwrap();
    assert_eq!(date_values.len(), 4);
    assert_eq!(date_values.get(0), Some(date(2019, 0, 1)));
    assert_eq!(date_values.get(1), Some(date(2018, 1, 2)));
    assert_eq!(date_values.get(2), Some(date(2017, 2, 3)));
    assert!(date_values.get(3).unwrap().is_null());

    assert_eq!(date_model.get_default(), Some(date(2019, 0, 1)));

    assert_eq!(messages(&result), vec!["attribute components.schemas.DateString.enum=`Nope` is not of type `date`".to_string()]);
}

#[test]
fn test_deserialize_date_time_string() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.0\n",
        "servers: []\n",
        "info:\n",
        "  version: 0.0.0\n",
        "  title: My Title\n",
        "paths:\n",
        "  /persons:\n",
        "    get:\n",
        "      description: a test\n",
        "      responses:\n",
        "        '200':\n",
        "          description: Successful response\n",
        "          content:\n",
        "            '*/*':\n",
        "              schema:\n",
        "                type: object\n",
        "                properties:\n",
        "                  dateTime:\n",
        "                    $ref: \"#/components/schemas/DateTimeString\"\n",
        "components:\n",
        "  schemas:\n",
        "    DateTimeString:\n",
        "      type: string\n",
        "      format: date-time\n",
        "      default: 2019-01-01T00:00:00Z\n",
        "      enum:\n",
        "        - null\n",
        "        - Nunh uh\n",
        "        - 2019-01-01T00:00:00Z\n",
        "        - 2018-02-02T23:59:59.999-05:00\n",
        "        - 2017-03-03T11:22:33+06:00\n",
        "        - 2016-04-04T22:33:44.555Z\n",
        ""
    );
    let result = read_contents(yaml);
    let resolved = resolve(result.openapi.clone());

    let date_time_model = schemas(&resolved).get("DateTimeString").unwrap();
    assert!(date_time_model.kind() == SchemaKind::DateTime);
    let values = date_time_model.get_enum().unwrap();
    assert_eq!(values.len(), 5);
    assert!(values.get(0).unwrap().is_null());
    assert_eq!(values.get(1), Some(odt(2019, 1, 1, 0, 0, 0, 0, 0)));
    assert_eq!(values.get(2), Some(odt(2018, 2, 2, 23, 59, 59, 999_000_000, -5 * 3600)));
    assert_eq!(values.get(3), Some(odt(2017, 3, 3, 11, 22, 33, 0, 6 * 3600)));
    assert_eq!(values.get(4), Some(odt(2016, 4, 4, 22, 33, 44, 555_000_000, 0)));

    assert_eq!(date_time_model.get_default(), Some(odt(2019, 1, 1, 0, 0, 0, 0, 0)));

    assert_eq!(messages(&result), vec!["attribute components.schemas.DateTimeString.enum=`Nunh uh` is not of type `date-time`".to_string()]);
}

#[test]
fn test_deserialize_byte_string() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.0\n",
        "servers: []\n",
        "info:\n",
        "  version: 0.0.0\n",
        "  title: My Title\n",
        "paths:\n",
        "  /persons:\n",
        "    get:\n",
        "      description: a test\n",
        "      responses:\n",
        "        '200':\n",
        "          description: Successful response\n",
        "          content:\n",
        "            '*/*':\n",
        "              schema:\n",
        "                type: object\n",
        "                properties:\n",
        "                  bytes:\n",
        "                    $ref: \"#/components/schemas/ByteString\"\n",
        "components:\n",
        "  schemas:\n",
        "    ByteString:\n",
        "      type: string\n",
        "      format: byte\n",
        "      default: W.T.F?\n",
        "      enum:\n",
        "        - VGhlIHdvcmxk\n",
        "        - aXMgYWxs\n",
        "        - dGhhdCBpcw==\n",
        "        - dGhlIGNhc2U=\n",
        "        - W.T.F?\n",
        ""
    );
    let result = read_contents(yaml);
    let resolved = resolve(result.openapi.clone());

    let byte_model = schemas(&resolved).get("ByteString").unwrap();
    assert!(byte_model.kind() == SchemaKind::ByteArray);
    let byte_values = byte_model.get_enum().unwrap();
    assert_eq!(byte_values.len(), 4);
    let text = |o: Option<JavaObject>| match o {
        Some(JavaObject::Bytes(b)) => String::from_utf8(b.to_vec()).unwrap(),
        other => panic!("not a byte[]: {other:?}"),
    };
    assert_eq!(text(byte_values.get(0)), "The world");
    assert_eq!(text(byte_values.get(1)), "is all");
    assert_eq!(text(byte_values.get(2)), "that is");
    assert_eq!(text(byte_values.get(3)), "the case");

    assert!(byte_model.get_default().is_none());

    assert_eq!(
        messages(&result),
        vec![
            "attribute components.schemas.ByteString.enum=`W.T.F?` is not of type `byte`".to_string(),
            "attribute components.schemas.ByteString.default=`W.T.F?` is not of type `byte`".to_string()
        ]
    );
}

#[test]
fn test_body_content() {
    let _g = common::setup(common::V3_MODULE);
    let json = concat!(
        "{",
        "  \"openapi\": \"3.0.0\",",
        "  \"info\": {",
        "    \"title\": \"Operations\",",
        "    \"version\": \"0.0.0\"",
        "  },",
        "  \"paths\": {",
        "    \"/operations\": {",
        "      \"post\": {",
        "        \"requestBody\": {",
        "            \"description\": \"Content empty\",",
        "            \"content\": {",
        "            }",
        "        },",
        "        \"responses\": {",
        "          \"default\": {",
        "            \"description\": \"None\"",
        "          }",
        "        }",
        "      },",
        "      \"put\": {",
        "        \"requestBody\": {",
        "            \"description\": \"Content undefined\"",
        "        },",
        "        \"responses\": {",
        "          \"default\": {",
        "            \"description\": \"None\"",
        "          }",
        "        }",
        "      }",
        "    }",
        "  }",
        "}"
    );
    let result = read_contents(json);
    let open_api = result.openapi.clone().unwrap();

    let post = path(&open_api, "/operations").get_post().unwrap();
    assert!(post.get_request_body().unwrap().get_content().is_none(), "Empty content");
    assert!(messages(&result).contains(&"attribute paths.'/operations'(post).requestBody.content with no media type is unsupported".to_string()), "Empty content error reported");

    let put = path(&open_api, "/operations").get_put().unwrap();
    assert!(put.get_request_body().unwrap().get_content().is_none(), "Empty content");
    assert!(messages(&result).contains(&"attribute paths.'/operations'(put).requestBody.content is missing".to_string()), "Missing content error reported");

    assert_eq!(messages(&result).len(), 2, "Messages");
}

#[test]
fn test_encoding_not_applicable() {
    let _g = common::setup(common::V3_MODULE);
    let json = concat!(
        "{",
        "  \"openapi\": \"3.0.0\",",
        "  \"info\": {",
        "    \"title\": \"Encodings\",",
        "    \"version\": \"0.0.0\"",
        "  },",
        "  \"paths\": {",
        "    \"/properties\": {",
        "      \"post\": {",
        "        \"requestBody\": {",
        "          \"content\": {",
        "            \"multipart/form-data\": {",
        "              \"schema\": {",
        "                \"type\": \"object\",",
        "                \"properties\": {",
        "                  \"X\": {",
        "                    \"type\": \"integer\"",
        "                  },",
        "                  \"Y\": {",
        "                    \"type\": \"integer\"",
        "                  }",
        "                }",
        "              },",
        "              \"encoding\": {",
        "                \"X\": {",
        "                  \"contentType\": \"application/json\"",
        "                },",
        "                \"Z\": {",
        "                  \"contentType\": \"text/plain\"",
        "                }",
        "              }",
        "            }",
        "          }",
        "        },",
        "        \"responses\": {",
        "          \"default\": {",
        "            \"description\": \"None\"",
        "          }",
        "        }",
        "      }",
        "    },",
        "    \"/propertiesNone\": {",
        "      \"post\": {",
        "        \"requestBody\": {",
        "          \"content\": {",
        "            \"multipart/form-data\": {",
        "              \"schema\": {",
        "                \"type\": \"object\"",
        "              },",
        "              \"encoding\": {",
        "                \"X\": {",
        "                  \"contentType\": \"application/json\"",
        "                },",
        "                \"Z\": {",
        "                  \"contentType\": \"text/plain\"",
        "                }",
        "              }",
        "            }",
        "          }",
        "        },",
        "        \"responses\": {",
        "          \"default\": {",
        "            \"description\": \"None\"",
        "          }",
        "        }",
        "      }",
        "    },",
        "    \"/schemaNone\": {",
        "      \"post\": {",
        "        \"requestBody\": {",
        "          \"content\": {",
        "            \"application/x-www-form-urlencoded\": {",
        "              \"encoding\": {",
        "                \"X\": {",
        "                  \"contentType\": \"application/json\"",
        "                },",
        "                \"Z\": {",
        "                  \"contentType\": \"text/plain\"",
        "                }",
        "              }",
        "            }",
        "          }",
        "        },",
        "        \"responses\": {",
        "          \"default\": {",
        "            \"description\": \"None\"",
        "          }",
        "        }",
        "      }",
        "    },",
        "    \"/schemaRef\": {",
        "      \"post\": {",
        "        \"requestBody\": {",
        "          \"content\": {",
        "            \"application/x-www-form-urlencoded\": {",
        "              \"schema\": {",
        "                \"$ref\": \"#/components/schemas/encoded\"",
        "              },",
        "              \"encoding\": {",
        "                \"X\": {",
        "                  \"contentType\": \"application/json\"",
        "                },",
        "                \"Z\": {",
        "                  \"contentType\": \"text/plain\"",
        "                }",
        "              }",
        "            }",
        "          }",
        "        },",
        "        \"responses\": {",
        "          \"default\": {",
        "            \"description\": \"None\"",
        "          }",
        "        }",
        "      }",
        "    }",
        "  },",
        "  \"components\": {",
        "    \"schemas\": {",
        "      \"encoded\": {",
        "        \"type\": \"object\",",
        "        \"properties\": {",
        "          \"X\": {",
        "            \"type\": \"integer\"",
        "          },",
        "          \"Y\": {",
        "            \"type\": \"integer\"",
        "          }",
        "        }",
        "      }",
        "    }",
        "  }",
        "}"
    );
    let result = read_contents(json);
    assert_eq!(
        messages(&result),
        vec![
            "attribute paths.'/properties'(post).requestBody.content.'multipart/form-data'.encoding.Z is unexpected".to_string(),
            "attribute paths.'/propertiesNone'(post).requestBody.content.'multipart/form-data'.encoding.X is unexpected".to_string(),
            "attribute paths.'/propertiesNone'(post).requestBody.content.'multipart/form-data'.encoding.Z is unexpected".to_string(),
            "attribute paths.'/schemaNone'(post).requestBody.content.'application/x-www-form-urlencoded'.encoding.X is unexpected".to_string(),
            "attribute paths.'/schemaNone'(post).requestBody.content.'application/x-www-form-urlencoded'.encoding.Z is unexpected".to_string(),
        ],
        "Error messages"
    );
}

#[test]
fn test_style_invalid() {
    let _g = common::setup(common::V3_MODULE);
    let json = concat!(
        "{",
        "    \"openapi\": \"3.0.0\",",
        "    \"info\": {",
        "        \"title\": \"realize\",",
        "        \"version\": \"0.0.0\"",
        "    },",
        "    \"paths\": {",
        "        \"/realize/{param}\": {",
        "            \"post\": {",
        "                \"parameters\": [",
        "                    {",
        "                        \"name\": \"param\",",
        "                        \"in\": \"path\",",
        "",
        "                        \"style\": \"DERP\",",
        "                        \"required\": true,",
        "",
        "                        \"schema\": {",
        "                            \"type\": \"string\",",
        "                            \"nullable\": false,",
        "                            \"minLength\": 1",
        "                        }",
        "                    }",
        "                ],",
        "                \"responses\": {",
        "                    \"200\": {",
        "                        \"description\": \"Success\",",
        "                        \"content\": {",
        "                            \"application/json\": {",
        "                                \"schema\": {",
        "                                    \"type\": \"object\"",
        "                                }",
        "                            }",
        "                        }",
        "                    }",
        "                }",
        "            }",
        "        }",
        "    }",
        "}"
    );
    let result = read_contents(json);
    assert!(messages(&result).len() == 1);
    assert_eq!(messages(&result)[0], "attribute paths.'/realize/{param}'(post).parameters.[param].style is not of type `StyleEnum`");
}

#[test]
fn test_param_content() {
    let _g = common::setup(common::V3_MODULE);
    let json = concat!(
        "{",
        "  \"openapi\": \"3.0.0\",",
        "  \"info\": {",
        "    \"title\": \"Operations\",",
        "    \"version\": \"0.0.0\"",
        "  },",
        "  \"paths\": {",
        "    \"/operations\": {",
        "      \"post\": {",
        "        \"parameters\": [",
        "          {",
        "            \"name\": \"param0\",",
        "            \"in\": \"query\",",
        "            \"content\": {",
        "            }",
        "          },",
        "          {",
        "            \"name\": \"param1\",",
        "            \"in\": \"query\",",
        "            \"content\": {",
        "              \"text/plain\": {",
        "              }",
        "            }",
        "          },",
        "          {",
        "            \"name\": \"param2\",",
        "            \"in\": \"query\",",
        "            \"content\": {",
        "              \"text/plain\": {",
        "              },",
        "              \"application/json\": {",
        "                \"schema\": {",
        "                  \"type\": \"object\"",
        "                }",
        "              }",
        "            }",
        "          }",
        "        ],",
        "        \"responses\": {",
        "          \"default\": {",
        "            \"description\": \"None\"",
        "          }",
        "        }",
        "      }",
        "    }",
        "  }",
        "}"
    );
    let result = read_contents(json);
    let post = path(&result.openapi.clone().unwrap(), "/operations").get_post().unwrap();

    let param0 = find_param(&post, "param0");
    assert!(messages(&result).contains(&"attribute paths.'/operations'(post).parameters.[param0].content with no media type is unsupported".to_string()), "No media types error reported");
    assert!(param0.get_content().is_none(), "Empty content");

    let param1 = find_param(&post, "param1");
    assert_eq!(param1.get_content().unwrap().len(), 1, "Valid content size");

    let param2 = find_param(&post, "param2");
    assert!(messages(&result).contains(&"attribute paths.'/operations'(post).parameters.[param2].content with multiple media types is unsupported".to_string()), "Multiple media types error reported");
    assert!(param2.get_content().is_none(), "Content with multiple media types");

    assert_eq!(messages(&result).len(), 2, "Messages");
}

#[test]
fn test_param_data() {
    let _g = common::setup(common::V3_MODULE);
    let json = concat!(
        "{",
        "  \"openapi\": \"3.0.0\",",
        "  \"info\": {",
        "    \"title\": \"Operations\",",
        "    \"version\": \"0.0.0\"",
        "  },",
        "  \"paths\": {",
        "    \"/operations\": {",
        "      \"post\": {",
        "        \"parameters\": [",
        "          {",
        "            \"name\": \"param0\",",
        "            \"in\": \"query\"",
        "          },",
        "          {",
        "            \"name\": \"param2\",",
        "            \"in\": \"query\",",
        "            \"content\": {",
        "              \"text/plain\": {",
        "              }",
        "            },",
        "            \"schema\": {",
        "               \"type\": \"object\"",
        "              }",
        "            }",
        "        ],",
        "        \"responses\": {",
        "          \"default\": {",
        "            \"description\": \"None\"",
        "          }",
        "        }",
        "      }",
        "    }",
        "  }",
        "}"
    );
    let result = read_contents(json);
    let post = path(&result.openapi.clone().unwrap(), "/operations").get_post().unwrap();

    let param0 = find_param(&post, "param0");
    assert!(messages(&result).contains(&"attribute paths.'/operations'(post).parameters.[param0].content is missing".to_string()), "No schema or content error reported");
    assert!(param0.get_content().is_none(), "No schema or content");

    let param2 = find_param(&post, "param2");
    assert!(messages(&result).contains(&"attribute paths.'/operations'(post).parameters.[param2].content when schema defined is unsupported".to_string()), "Both schema and content error reported");
    assert!(param2.get_content().is_none(), "Content when schema defined");

    assert_eq!(messages(&result).len(), 2, "Messages");
}

#[test]
fn test_deserialize_with_messages() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: '3.0.0'\n",
        "info:\n",
        "  version: 0.0.0\n",
        "  title:\n",
        "    - bar"
    );
    let result = read_contents(yaml);
    let messages = message_set(&result);
    assert!(messages.len() == 2);
    assert!(messages.contains("attribute info.title is not of type `string`"));
    assert!(messages.contains("attribute paths is missing"));
}

#[test]
fn test_deserialize_with_discriminator() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.0\n",
        "servers: []\n",
        "info:\n",
        "  version: ''\n",
        "  title: ''\n",
        "paths: {}\n",
        "components:\n",
        "  schemas:\n",
        "    Animal:\n",
        "      type: object\n",
        "      discriminator:\n",
        "        propertyName: petType\n",
        "      description: |\n",
        "        A basic `Animal` object which can extend to other animal types.\n",
        "      required:\n",
        "        - commonName\n",
        "        - petType\n",
        "      properties:\n",
        "        commonName:\n",
        "          description: the household name of the animal\n",
        "          type: string\n",
        "        petType:\n",
        "          description: |\n",
        "            The discriminator for the animal type.  It _must_\n",
        "            match one of the concrete schemas by name (i.e. `Cat`)\n",
        "            for proper deserialization\n",
        "          type: string"
    );
    let result = read_contents(yaml);
    assert!(!message_set(&result).contains("attribute definitions.Animal.discriminator is unexpected"));
}

#[test]
fn test_deserialize_with_enum_discriminator() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.0\n",
        "components:\n",
        "  schemas:\n",
        "    Animal:\n",
        "      type: object\n",
        "      discriminator:\n",
        "        propertyName: petType\n",
        "      description: |\n",
        "        A basic `Animal` object which can extend to other animal types.\n",
        "      required:\n",
        "        - commonName\n",
        "        - petType\n",
        "      properties:\n",
        "        commonName:\n",
        "          description: the household name of the animal\n",
        "          type: string\n",
        "        petType:\n",
        "          description: |\n",
        "            The discriminator for the animal type.  It _must_\n",
        "            match one of the concrete schemas by name (i.e. `Cat`)\n",
        "            for proper deserialization\n",
        "          enum:\n",
        "            - cat\n",
        "            - dog"
    );
    let result = read_contents(yaml);
    let properties = schemas(&result.openapi.clone().unwrap()).get("Animal").unwrap().get_properties().unwrap();
    assert!(properties.contains_key("commonName"));
    assert!(properties.contains_key("petType"));
    assert_eq!(properties.get("petType").unwrap().get_type().as_deref(), Some("string"));
}

#[test]
fn test_deserialize_with_numeric_enum_discriminator() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.0\n",
        "components:\n",
        "  schemas:\n",
        "    Animal:\n",
        "      type: object\n",
        "      discriminator:\n",
        "        propertyName: petType\n",
        "      description: |\n",
        "        A basic `Animal` object which can extend to other animal types.\n",
        "      required:\n",
        "        - commonName\n",
        "        - petType\n",
        "      properties:\n",
        "        commonName:\n",
        "          description: the household name of the animal\n",
        "          type: string\n",
        "        petType:\n",
        "          enum:\n",
        "            - 1\n",
        "            - 2"
    );
    let result = read_contents(yaml);
    let properties = schemas(&result.openapi.clone().unwrap()).get("Animal").unwrap().get_properties().unwrap();
    assert!(properties.contains_key("commonName"));
    assert!(properties.contains_key("petType"));
    assert_eq!(properties.get("petType").unwrap().get_type().as_deref(), Some("number"));
}

#[test]
fn test_deserialize_with_boolean_enum_discriminator() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.0\n",
        "components:\n",
        "  schemas:\n",
        "    Animal:\n",
        "      type: object\n",
        "      discriminator:\n",
        "        propertyName: petType\n",
        "      description: |\n",
        "        A basic `Animal` object which can extend to other animal types.\n",
        "      required:\n",
        "        - commonName\n",
        "        - petType\n",
        "      properties:\n",
        "        commonName:\n",
        "          description: the household name of the animal\n",
        "          type: string\n",
        "        petType:\n",
        "          enum:\n",
        "            - true\n",
        "            - false"
    );
    let result = read_contents(yaml);
    let properties = schemas(&result.openapi.clone().unwrap()).get("Animal").unwrap().get_properties().unwrap();
    assert!(properties.contains_key("commonName"));
    assert!(properties.contains_key("petType"));
    assert_eq!(properties.get("petType").unwrap().get_type().as_deref(), Some("boolean"));
}

#[test]
fn test_issue161() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.0\n",
        "servers: []\n",
        "paths:\n",
        "  /users:\n",
        "    get:\n",
        "      parameters:\n",
        "        - in: query\n",
        "          name: name\n",
        "          required: false\n",
        "          schema:\n",
        "            type: string\n",
        "            minLength: 10\n",
        "            maxLength: 100\n",
        "      responses:\n",
        "        default:\n",
        "          description: ok\n"
    );
    let result = read_contents(yaml);
    assert!(!message_set(&result).contains("attribute paths.'/users'(get).[name].maxLength is unexpected"));
}

#[test]
fn test_validator_issue50() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.0\n",
        "servers:\n",
        "  - url: 'http://local.xxx.com/'\n",
        "info:\n",
        "  version: 2.0.0\n",
        "  title: Beanhunter API\n",
        "  description: Description of the api goes here.\n",
        "paths:\n",
        "  /city:\n",
        "    get:\n",
        "      description: test description\n",
        "components:\n",
        "  schemas:\n",
        "    Endpoints:\n",
        "      title: Endpoints object\n",
        "      properties:\n",
        "        links: {}"
    );
    let result = read_contents(yaml);
    assert!(messages(&result).len() == 1);
}

#[test]
fn test_issue151() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.0\n",
        "servers: []\n",
        "info:\n",
        "  version: 2.0.0\n",
        "  title: Test Issue 151\n",
        "  description: Tests that ComposedSchema vendor extensions are deserialized correctly.\n",
        "paths:\n",
        "  /:\n",
        "    get:\n",
        "      responses:\n",
        "        '200':\n",
        "          description: OK\n",
        "components:\n",
        "  schemas:\n",
        "    Pet:\n",
        "      type: object\n",
        "      required:\n",
        "        - id\n",
        "      properties:\n",
        "        id:\n",
        "          type: integer\n",
        "          format: int64\n",
        "    Dog:\n",
        "      type: object\n",
        "      allOf:\n",
        "        - $ref: \"#/components/schemas/Pet\"\n",
        "        - required:\n",
        "            - name\n",
        "          properties:\n",
        "            name:\n",
        "              type: string\n",
        "      x-vendor-ext: some data"
    );
    let result = read_contents(yaml);
    assert!(messages(&result).is_empty());
    let open_api = result.openapi.clone().unwrap();

    let definitions = schemas(&open_api);
    assert!(definitions.len() == 2);
    let all_of_model = definitions.get("Dog").unwrap();
    assert!(all_of_model.kind() == SchemaKind::Composed);
    assert!(!all_of_model.get_extensions().unwrap().is_empty());
    assert_eq!(Some(s("some data")), all_of_model.get_extensions().unwrap().get("x-vendor-ext"));
}

#[test]
fn test_issue204_all_of() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.0\n",
        "servers: []\n",
        "info:\n",
        "  version: 2.0.0\n",
        "  title: Test allOf API\n",
        "  description: 'Tests the allOf API for parent, interface and child models.'\n",
        "paths:\n",
        "  /:\n",
        "    get:\n",
        "      responses:\n",
        "        '200':\n",
        "          description: OK\n",
        "components:\n",
        "  schemas:\n",
        "    Pet:\n",
        "      type: object\n",
        "      required:\n",
        "        - id\n",
        "      properties:\n",
        "        id:\n",
        "          type: integer\n",
        "          format: int64\n",
        "    Furry:\n",
        "      type: object\n",
        "      required:\n",
        "        - coatColour\n",
        "      properties:\n",
        "        coatColour:\n",
        "          type: string\n",
        "    Dog:\n",
        "      type: object\n",
        "      allOf:\n",
        "        - $ref: \"#/components/schemas/Pet\"\n",
        "        - $ref: \"#/components/schemas/Furry\"\n",
        "        - required:\n",
        "            - name\n",
        "          properties:\n",
        "            name:\n",
        "              type: string"
    );
    let result = read_contents(yaml);
    assert!(messages(&result).is_empty());
    let open_api = result.openapi.clone();
    assert!(open_api.is_some());
    let open_api = open_api.unwrap();

    let definitions = open_api.get_components().unwrap().get_schemas();
    assert!(definitions.is_some());
    let definitions = definitions.unwrap();
    assert_eq!(3, definitions.len());

    let pet = definitions.get("Pet");
    let furry = definitions.get("Furry");
    let dog = definitions.get("Dog");

    assert!(pet.is_some());
    assert!(furry.is_some());
    assert!(dog.is_some());
    let dog_composed = dog.unwrap();
    assert!(dog_composed.kind() == SchemaKind::Composed);
    assert!(dog_composed.get_all_of().is_some());
    assert_eq!(3, dog_composed.get_all_of().unwrap().len());
    let dog_interface_ref = dog_composed.get_all_of().unwrap().get(0).unwrap();
    let _dog_interface = definitions.get(&dog_interface_ref.get_ref().unwrap());
    let dog_interface_ref = dog_composed.get_all_of().unwrap().get(1).unwrap();
    let _dog_interface = definitions.get(&dog_interface_ref.get_ref().unwrap());
    assert!(dog_composed.get_all_of().unwrap().get(0).unwrap().get_ref().is_some());
}

#[test]
fn test_pr246() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.0\n",
        "servers: []\n",
        "info:\n",
        "  description: 'Tests the allOf API for parent, interface and child models.'\n",
        "  version: 2.0.0\n",
        "  title: Test allOf API\n",
        "paths:\n",
        "  /:\n",
        "    get:\n",
        "      responses:\n",
        "        '200':\n",
        "          description: OK\n",
        "    parameters: []\n",
        "components:\n",
        "  schemas:\n",
        "    Pet:\n",
        "      type: object\n",
        "      required:\n",
        "        - id\n",
        "      properties:\n",
        "        id:\n",
        "          type: integer\n",
        "          format: int64\n",
        "    Furry:\n",
        "      type: object\n",
        "      required:\n",
        "        - coatColour\n",
        "      properties:\n",
        "        coatColour:\n",
        "          type: string\n",
        "    Dog:\n",
        "      allOf:\n",
        "        - $ref: \"#/components/schemas/Pet\"\n",
        "        - $ref: \"#/components/schemas/Furry\"\n",
        "        - type: object\n",
        "          required:\n",
        "            - name\n",
        "          properties:\n",
        "            name:\n",
        "              type: string"
    );
    let result = read_contents(yaml);
    let open_api = result.openapi.clone().unwrap();

    let dog = schemas(&open_api).get("Dog");
    assert!(dog.is_some());
    let composed = dog.unwrap();
    assert!(composed.kind() == SchemaKind::Composed);

    assert!(composed.get_all_of().unwrap().get(0).unwrap().get_ref().is_some());
    assert!(composed.get_all_of().unwrap().len() == 3);
}

#[test]
fn test_issue247() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.0\n",
        "servers: []\n",
        "info:\n",
        "  description: bleh\n",
        "  version: 2.0.0\n",
        "  title: Test\n",
        "paths:\n",
        "  /:\n",
        "    get:\n",
        "      responses:\n",
        "        '200':\n",
        "          description: OK\n",
        "    parameters: []\n",
        "components:\n",
        "  schemas:\n",
        "    Pet:\n",
        "      allOf:\n",
        "        - type: object\n",
        "          required:\n",
        "            - id\n",
        "          properties:\n",
        "            id:\n",
        "              type: integer\n",
        "              format: int64"
    );
    let result = read_contents(yaml);
    let open_api = result.openapi.clone().unwrap();
    assert!(schemas(&open_api).get("Pet").is_some());
}

#[test]
fn test_issue343_parameter() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.0\n",
        "servers: []\n",
        "info:\n",
        "  description: bleh\n",
        "  version: 2.0.0\n",
        "  title: Test\n",
        "paths:\n",
        "  /foo:\n",
        "    post:\n",
        "      parameters:\n",
        "        - in: query\n",
        "          name: skip\n",
        "          schema:\n",
        "            type: integer\n",
        "            format: int32\n",
        "            multipleOf: 3\n",
        "      responses:\n",
        "        '200':\n",
        "          description: OK\n",
        "      requestBody:\n",
        "        content:\n",
        "          application/json:\n",
        "            schema:\n",
        "              type: object\n",
        "              additionalProperties:\n",
        "                type: string\n",
        "        required: true\n",
        "components:\n",
        "  schemas:\n",
        "    Fun:\n",
        "      properties:\n",
        "        id:\n",
        "          type: integer\n",
        "          format: int32\n",
        "          multipleOf: 5\n",
        "        mySet:\n",
        "          type: array\n",
        "          uniqueItems: true\n",
        "          items:\n",
        "            type: string"
    );
    let result = read_contents(yaml);
    let open_api = result.openapi.clone().unwrap();

    let post = path(&open_api, "/foo").get_post().unwrap();
    let qp = post.get_parameters().unwrap().get(0).unwrap();
    assert!(qp.kind() == ParameterKind::Query, "(QueryParameter) cast");
    assert_eq!(Some(big("3")), qp.get_schema().unwrap().get_multiple_of());

    let bp = post.get_request_body().unwrap();
    let schema = bp.get_content().unwrap().get("application/json").unwrap().get_schema().unwrap();
    assert!(schema.get_additional_properties().is_some());

    let fun = schemas(&open_api).get("Fun").unwrap().get_properties().unwrap();
    let id = fun.get("id").unwrap();
    assert!(id.kind() == SchemaKind::Integer, "(IntegerSchema) cast");
    assert_eq!(id.get_multiple_of(), Some(big("5")));

    let ap = fun.get("mySet").unwrap();
    assert!(ap.kind() == SchemaKind::Array, "(ArraySchema) cast");
    assert!(ap.get_unique_items().unwrap());
}

#[test]
fn test_issue386() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.0\n",
        "servers: []\n",
        "info:\n",
        "  description: bleh\n",
        "  version: 2.0.0\n",
        "  title: Test\n",
        "paths:\n",
        "  /foo:\n",
        "    post:\n",
        "      responses:\n",
        "        '200':\n",
        "          description: OK\n",
        "      requestBody:\n",
        "        content:\n",
        "          application/json:\n",
        "            schema:\n",
        "              type: object\n",
        "              enum:\n",
        "                - id: fun\n",
        "              properties:\n",
        "                id:\n",
        "                  type: string\n",
        "components:\n",
        "  schemas:\n",
        "    Fun:\n",
        "      type: object\n",
        "      properties:\n",
        "        complex:\n",
        "          enum:\n",
        "            - id: 110\n",
        "          type: object\n",
        "          properties:\n",
        "            id:\n",
        "              type: string\n",
        "    MyEnum:\n",
        "      type: integer\n",
        "      enum:\n",
        "        - value: 3\n",
        "          description: Value 1\n",
        "        - value: 10\n",
        "          description: Value 2"
    );
    let result = read_contents(yaml);
    assert!(result.openapi.is_some());
}

#[test]
fn test_issue360() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPI::new();

    let model = Schema::new();
    model.set_enum(None::<JList<JavaObject>>);
    open_api.set_components(Components::new().add_schemas("modelWithNullEnum", model));

    let json = json_pretty(&open_api).unwrap();

    let result = read_contents(&json);
    let rebuilt = result.openapi.clone();
    assert!(rebuilt.is_some());
}

#[test]
fn test_all_of_schema() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: '3.0'\n",
        "components:\n",
        "  schemas:\n",
        "    Pet:\n",
        "      type: object\n",
        "      required:\n",
        "      - pet_type\n",
        "      properties:\n",
        "        pet_type:\n",
        "          type: string\n",
        "    Cat:\n",
        "      allOf:\n",
        "      - $ref: \"#/components/schemas/Pet\"\n",
        "      - type: object\n",
        "        # all other properties specific to a `Cat`\n",
        "        properties:\n",
        "          name:\n",
        "            type: string\n"
    );
    let result = read_contents_resolve(yaml);
    let _messages = message_set(&result);

    let cat_schema = schemas(&result.openapi.clone().unwrap()).get("Cat");
    assert!(cat_schema.is_some());
    let cat_comp_schema = cat_schema.unwrap();
    assert!(cat_comp_schema.kind() == SchemaKind::Composed);

    let all_of_schemas = cat_comp_schema.get_all_of();
    assert!(all_of_schemas.is_some());
    let all_of_schemas = all_of_schemas.unwrap();
    assert_eq!(all_of_schemas.len(), 2);

    let ref_pet_schema = all_of_schemas.get(0).unwrap();
    assert_eq!(ref_pet_schema.get_ref().as_deref(), Some("#/components/schemas/Pet"));

    let other_schema = all_of_schemas.get(1).unwrap();
    assert!(other_schema.get_properties().is_some());
    let name_prop = other_schema.get_properties().unwrap().get("name");
    assert!(name_prop.is_some());
    assert_eq!(name_prop.unwrap().get_type().as_deref(), Some("string"));
}

#[test]
fn test_one_of_schema() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: '3.0'\n",
        "components:\n",
        "  schemas:\n",
        "    Cat:\n",
        "      type: object\n",
        "      # all properties specific to a `Cat`\n",
        "      properties:\n",
        "        purring:\n",
        "          type: string\n",
        "    Dog:\n",
        "      type: object\n",
        "      # all properties specific to a `Dog`\n",
        "      properties:\n",
        "        bark:\n",
        "          type: string\n",
        "    Pet:\n",
        "      oneOf: \n",
        "       - $ref: \"#/components/schemas/Cat\"\n",
        "       - $ref: \"#/components/schemas/Dog\"\n",
        "       - type: object\n",
        "         # neither a `Cat` nor a `Dog`\n",
        "         properties:\n",
        "           name:\n",
        "             type: string\n"
    );
    let result = read_contents_resolve(yaml);
    let _messages = message_set(&result);

    let pet_schema = schemas(&result.openapi.clone().unwrap()).get("Pet");
    assert!(pet_schema.is_some());
    let pet_comp_schema = pet_schema.unwrap();
    assert!(pet_comp_schema.kind() == SchemaKind::Composed);

    let one_of_schemas = pet_comp_schema.get_one_of();
    assert!(one_of_schemas.is_some());
    let one_of_schemas = one_of_schemas.unwrap();
    assert_eq!(one_of_schemas.len(), 3);

    let ref_cat_schema = one_of_schemas.get(0).unwrap();
    assert_eq!(ref_cat_schema.get_ref().as_deref(), Some("#/components/schemas/Cat"));

    let ref_dog_schema = one_of_schemas.get(1).unwrap();
    assert_eq!(ref_dog_schema.get_ref().as_deref(), Some("#/components/schemas/Dog"));

    let other_schema = one_of_schemas.get(2).unwrap();
    let name_prop = other_schema.get_properties().unwrap().get("name");
    assert!(name_prop.is_some());
    assert_eq!(name_prop.unwrap().get_type().as_deref(), Some("string"));
}

#[test]
fn test_any_of_schema() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: '3.0'\n",
        "components:\n",
        "  schemas:\n",
        "    id:\n",
        "      anyOf: \n",
        "       - type: string\n",
        "       - type: number\n"
    );
    let result = read_contents_resolve(yaml);
    let _messages = message_set(&result);

    let id_schema = schemas(&result.openapi.clone().unwrap()).get("id");
    assert!(id_schema.is_some());
    let id_comp_schema = id_schema.unwrap();
    assert!(id_comp_schema.kind() == SchemaKind::Composed);

    let any_of_schemas = id_comp_schema.get_any_of();
    assert!(any_of_schemas.is_some());
    let any_of_schemas = any_of_schemas.unwrap();
    assert_eq!(any_of_schemas.len(), 2);

    let string_schema = any_of_schemas.get(0).unwrap();
    assert_eq!(string_schema.get_type().as_deref(), Some("string"));

    let number_schema = any_of_schemas.get(1).unwrap();
    assert_eq!(number_schema.get_type().as_deref(), Some("number"));
}

#[test]
fn property_test() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.1\n",
        "paths:\n",
        "  /primitiveBody/inline:\n",
        "    post:\n",
        "      x-swagger-router-controller: TestController\n",
        "      operationId: inlineRequiredBody\n",
        "      requestBody:\n",
        "        content:\n",
        "          application/json:\n",
        "            schema:\n",
        "              type: object\n",
        "              properties:\n",
        "                name:\n",
        "                  type: string\n",
        "        required: true\n",
        "      responses:\n",
        "        '200':\n",
        "          description: ok!"
    );
    let result = read_contents_resolve(yaml);
    let open_api = result.openapi.clone().unwrap();
    let properties = path(&open_api, "/primitiveBody/inline").get_post().unwrap().get_request_body().unwrap().get_content().unwrap().get("application/json").unwrap().get_schema().unwrap().get_properties().unwrap();
    assert!(properties.get("name").unwrap().kind() == SchemaKind::String);
}

#[test]
fn test_examples() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.1\n",
        "info:\n",
        "  title: httpbin\n",
        "  version: 0.0.0\n",
        "servers:\n",
        "  - url: http://httpbin.org\n",
        "paths:\n",
        "  /post:\n",
        "    post:\n",
        "      summary: Returns the POSTed data\n",
        "      requestBody:\n",
        "        content:\n",
        "          application/json:\n",
        "            schema:\n",
        "              $ref: \"#/components/schemas/AnyValue\"\n",
        "            examples:\n",
        "              AnObject:\n",
        "                $ref: \"#/components/examples/AnObject\"\n",
        "              ANull:\n",
        "                $ref: \"#/components/examples/ANull\"\n",
        "          application/yaml:\n",
        "            schema:\n",
        "              $ref: \"#/components/schemas/AnyValue\"\n",
        "            examples:\n",
        "              AString:\n",
        "                $ref: \"#/components/examples/AString\"\n",
        "              AnArray:\n",
        "                $ref: \"#/components/examples/AnArray\"\n",
        "          text/plain:\n",
        "            schema:\n",
        "              type: string\n",
        "              example: Hi there\n",
        "          application/x-www-form-urlencoded:\n",
        "            schema:\n",
        "              type: object\n",
        "              properties:\n",
        "                id:\n",
        "                  type: integer\n",
        "                name:\n",
        "                  type: string\n",
        "            example:\n",
        "              id: 42\n",
        "              name: Arthur Dent\n",
        "      responses:\n",
        "        '200':\n",
        "          description: OK\n",
        "          content:\n",
        "            application/json:\n",
        "              schema:\n",
        "                type: object\n",
        "\n",
        "  #/response-headers:\n",
        "  /:\n",
        "    get:\n",
        "      summary: Returns a response with the specified headers\n",
        "      parameters:\n",
        "        - in: header\n",
        "          name: Server\n",
        "          required: true\n",
        "          schema:\n",
        "            type: string\n",
        "          examples:\n",
        "            httpbin:\n",
        "              value: httpbin\n",
        "            unicorn:\n",
        "              value: unicorn\n",
        "        - in: header\n",
        "          name: X-Request-Id\n",
        "          required: true\n",
        "          schema:\n",
        "            type: integer\n",
        "          example: 37\n",
        "      responses:\n",
        "        '200':\n",
        "          description: A response with the specified headers\n",
        "          headers:\n",
        "            Server:\n",
        "              schema:\n",
        "                type: string\n",
        "              examples:\n",
        "                httpbin:\n",
        "                  value: httpbin\n",
        "                unicorn:\n",
        "                  value: unicorn\n",
        "            X-Request-Id:\n",
        "              schema:\n",
        "                type: integer\n",
        "              example: 37\n",
        "\n",
        "components:\n",
        "  schemas:\n",
        "    AnyValue:\n",
        "      nullable: true\n",
        "      description: Can be anything - string, object, array, null, etc.\n",
        "\n",
        "  examples:\n",
        "    AString:\n",
        "      value: Hi there\n",
        "    ANumber:\n",
        "      value: 42\n",
        "    ANull:\n",
        "      value: null\n",
        "    AnArray:\n",
        "      value: [1, 2, 3]\n",
        "    AnObject:\n",
        "      value:\n",
        "        id:  42\n",
        "        name: Arthur Dent"
    );
    let result = read_contents_resolve(yaml);
    let open_api = result.openapi.clone().unwrap();
    let media_type_json = path(&open_api, "/post").get_post().unwrap().get_request_body().unwrap().get_content().unwrap().get("application/json").unwrap();
    let get = path(&open_api, "/").get_get().unwrap();
    let headers = get.get_responses().unwrap().get("200").unwrap().get_headers().unwrap();
    let header1 = headers.get("Server").unwrap();
    let header2 = headers.get("X-Request-Id").unwrap();
    let parameter1 = get.get_parameters().unwrap().get(0).unwrap();
    let parameter2 = get.get_parameters().unwrap().get(1).unwrap();

    assert!(media_type_json.get_examples().is_some());
    assert_eq!(media_type_json.get_examples().unwrap().get("AnObject").unwrap().get_ref().as_deref(), Some("#/components/examples/AnObject"));

    assert!(header1.get_examples().is_some());
    assert_eq!(header1.get_examples().unwrap().get("httpbin").unwrap().get_value(), Some(s("httpbin")));

    assert!(header2.get_example().is_some());
    assert_eq!(header2.get_example(), Some(JavaObject::Integer(37)));

    assert!(parameter1.get_examples().is_some());
    assert_eq!(parameter1.get_examples().unwrap().get("unicorn").unwrap().get_value(), Some(s("unicorn")));

    assert!(parameter2.get_example().is_some());
    assert_eq!(parameter2.get_example(), Some(JavaObject::Integer(37)));
}

#[test]
fn test_schema_example() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: '3.0.1'\n",
        "components:\n",
        "  schemas:\n",
        "    Address:\n",
        "      required:\n",
        "      - street\n",
        "      type: object\n",
        "      x-swagger-router-model: io.swagger.oas.test.models.Address\n",
        "      properties:\n",
        "        street:\n",
        "          type: string\n",
        "          example: 12345 El Monte Road\n",
        "        city:\n",
        "          type: string\n",
        "          example: Los Altos Hills\n",
        "        state:\n",
        "          type: string\n",
        "          example: CA\n",
        "        zip:\n",
        "          type: string\n",
        "          example: '94022'"
    );
    let result = read_contents_resolve(yaml);
    let open_api = result.openapi.clone().unwrap();
    let state_schema_property = schemas(&open_api).get("Address").unwrap().get_properties().unwrap().get("state").unwrap();

    assert!(state_schema_property.get_example().is_some());
    assert_eq!(state_schema_property.get_example(), Some(s("CA")));
}

#[test]
fn test_example_vs_examples() {
    let _g = common::setup(common::V3_MODULE);
    let json = concat!(
        "{",
        "\"openapi\": \"3.0.0\",",
        "\"info\": {\"title\": \"Examples\", \"version\": \"0.0.0\"},",
        "\"paths\": {},",
        "\"components\": {",
        "  \"parameters\": {",
        "    \"withExample\": {",
        "      \"name\": \"withExample\",",
        "      \"in\": \"query\",",
        "      \"schema\": {\"type\": \"string\"},",
        "      \"example\": \"Hello\"},",
        "    \"withExamples\": {",
        "      \"name\": \"withExamples\",",
        "      \"in\": \"query\",",
        "      \"schema\": {\"type\": \"string\"},",
        "      \"examples\": {\"Texan\": {\"value\": \"Howdy\"}}},",
        "    \"withBoth\": {",
        "      \"name\": \"withBoth\",",
        "      \"in\": \"query\",",
        "      \"schema\": {\"type\": \"string\"},",
        "      \"examples\": {\"Texan\": {\"value\": \"Howdy\"}},",
        "      \"example\": \"Hello\"},",
        "    \"withContentExample\": {",
        "      \"name\": \"withContentExample\",",
        "      \"in\": \"query\",",
        "      \"content\": {",
        "        \"application/json\": {",
        "          \"schema\": {\"type\": \"string\"},",
        "          \"example\": \"Hello\"}}},",
        "    \"withContentExamples\": {",
        "      \"name\": \"withContentExamples\",",
        "      \"in\": \"query\",",
        "      \"content\": {",
        "        \"application/json\": {",
        "          \"schema\": {\"type\": \"string\"},",
        "          \"examples\": {\"Texan\": {\"value\": \"Howdy\"}}}}},",
        "    \"withContentBoth\": {",
        "      \"name\": \"withContentBoth\",",
        "      \"in\": \"query\",",
        "      \"content\": {",
        "        \"application/json\": {",
        "          \"schema\": {\"type\": \"string\"},",
        "          \"example\": \"Hello\",",
        "          \"examples\": {\"Texan\": {\"value\": \"Howdy\"}}}}}},",
        "  \"headers\": {",
        "    \"withExample\": {",
        "      \"schema\": {\"type\": \"string\"},",
        "      \"example\": \"Hello\"},",
        "    \"withExamples\": {",
        "      \"schema\": {\"type\": \"string\"},",
        "      \"examples\": {\"Texan\": {\"value\": \"Howdy\"}}},",
        "    \"withBoth\": {",
        "      \"schema\": {\"type\": \"string\"},",
        "      \"examples\": {\"Texan\": {\"value\": \"Howdy\"}},",
        "      \"example\": \"Hello\"}},",
        "  \"requestBodies\": {",
        "    \"withBodyExample\": {",
        "      \"content\": {",
        "        \"application/json\": {",
        "          \"schema\": {\"type\": \"string\"},",
        "          \"example\": \"Hello\"}}},",
        "    \"withBodyExamples\": {",
        "      \"content\": {",
        "        \"application/json\": {",
        "          \"schema\": {\"type\": \"string\"},",
        "          \"examples\": {\"Texan\": {\"value\": \"Howdy\"}}}}},",
        "    \"withBodyBoth\": {",
        "      \"content\": {",
        "        \"application/json\": {",
        "          \"schema\": {\"type\": \"string\"},",
        "          \"example\": \"Hello\",",
        "          \"examples\": {\"Texan\": {\"value\": \"Howdy\"}}}}}}}}"
    );
    let result = read_contents(json);
    let mut actual = messages(&result);
    let mut expected = vec![
        "components.parameters.withBoth.[withBoth].examples already defined -- ignoring \"example\" field".to_string(),
        "components.parameters.withContentBoth.[withContentBoth].content.'application/json'.examples already defined -- ignoring \"example\" field".to_string(),
        "components.requestBodies.withBodyBoth.content.'application/json'.examples already defined -- ignoring \"example\" field".to_string(),
        "components.headers.withBoth.examples already defined -- ignoring \"example\" field".to_string(),
    ];
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected, "Expected warnings not found");

    let open_api = result.openapi.clone().unwrap();
    let components = open_api.get_components().unwrap();

    let param = components.get_parameters().unwrap().get("withExample").unwrap();
    assert!(param.get_examples().is_none(), "Examples,");
    assert!(param.get_example().is_some(), "Example,");

    let param = components.get_parameters().unwrap().get("withExamples").unwrap();
    assert!(param.get_examples().is_some(), "Examples,");
    assert!(param.get_example().is_none(), "Example,");

    let param = components.get_parameters().unwrap().get("withBoth").unwrap();
    assert!(param.get_examples().is_some(), "Examples,");
    assert!(param.get_example().is_none(), "Example,");

    let header = components.get_headers().unwrap().get("withExample").unwrap();
    assert!(header.get_examples().is_none(), "Examples,");
    assert!(header.get_example().is_some(), "Example,");

    let header = components.get_headers().unwrap().get("withExamples").unwrap();
    assert!(header.get_examples().is_some(), "Examples,");
    assert!(header.get_example().is_none(), "Example,");

    let header = components.get_headers().unwrap().get("withBoth").unwrap();
    assert!(header.get_examples().is_some(), "Examples,");
    assert!(header.get_example().is_none(), "Example,");

    let media_type = components.get_parameters().unwrap().get("withContentExample").unwrap().get_content().unwrap().get("application/json").unwrap();
    assert!(media_type.get_examples().is_none(), "Examples,");
    assert!(media_type.get_example().is_some(), "Example,");

    let media_type = components.get_parameters().unwrap().get("withContentExamples").unwrap().get_content().unwrap().get("application/json").unwrap();
    assert!(media_type.get_examples().is_some(), "Examples,");
    assert!(media_type.get_example().is_none(), "Example,");

    let media_type = components.get_parameters().unwrap().get("withContentBoth").unwrap().get_content().unwrap().get("application/json").unwrap();
    assert!(media_type.get_examples().is_some(), "Examples,");
    assert!(media_type.get_example().is_none(), "Example,");

    let media_type = components.get_request_bodies().unwrap().get("withBodyExample").unwrap().get_content().unwrap().get("application/json").unwrap();
    assert!(media_type.get_examples().is_none(), "Examples,");
    assert!(media_type.get_example().is_some(), "Example,");

    let media_type = components.get_request_bodies().unwrap().get("withBodyExamples").unwrap().get_content().unwrap().get("application/json").unwrap();
    assert!(media_type.get_examples().is_some(), "Examples,");
    assert!(media_type.get_example().is_none(), "Example,");

    let media_type = components.get_request_bodies().unwrap().get("withBodyBoth").unwrap().get_content().unwrap().get("application/json").unwrap();
    assert!(media_type.get_examples().is_some(), "Examples,");
    assert!(media_type.get_example().is_none(), "Example,");
}

fn enum_property_default(o: &OpenAPI, schema: &str) -> Option<JavaObject> {
    schemas(o).get(schema).unwrap().get_properties().unwrap().get("enumProperty").unwrap().get_default()
}

#[test]
fn test_issue1454_all_of_default_value() {
    let _g = common::setup(common::V3_MODULE);
    let json = concat!(
        "{",
        "  \"openapi\": \"3.0.1\",",
        "  \"info\": {",
        "    \"title\": \"Here be enum bugs\",",
        "    \"description\": \"OpenAPI spec to test the enum parsing bugs.\",",
        "    \"version\": \"1.0.0\"",
        "  },",
        "  \"paths\": {",
        "    \"/enum/parse/bug\": {",
        "      \"post\": {",
        "        \"summary\": \"Test enum parse bug\",",
        "        \"requestBody\": {",
        "          \"content\": {",
        "            \"application/json\": {",
        "              \"schema\": {",
        "                \"$ref\": \"#/components/schemas/SchemaWithDefaultAndEnum\"",
        "              }",
        "            }",
        "          }",
        "        },",
        "        \"responses\": {",
        "          \"default\": {",
        "            \"description\": \"Successful response\"",
        "          }",
        "        }",
        "      }",
        "    }",
        "  },",
        "  \"components\": {",
        "    \"schemas\": {",
        "      \"SchemaWithDefaultAndEnum\": {",
        "        \"type\": \"object\",",
        "        \"properties\": {",
        "          \"enumProperty\": {",
        "            \"description\": \"This schema with set default enum value and enum ref shows default SCHEMA_DEFAULT value in Swagger Editor, but is null when parsed with OpenAPIV3Parser.\",",
        "            \"allOf\": [",
        "              {",
        "                \"$ref\": \"#/components/schemas/Enum\"",
        "              }",
        "            ],",
        "            \"default\": \"SCHEMA_DEFAULT\"",
        "          }",
        "        }",
        "      },",
        "      \"Enum\": {",
        "        \"type\": \"string\",",
        "        \"enum\": [",
        "          \"SCHEMA_DEFAULT\",",
        "          \"NOT_DEFAULT\"",
        "        ]",
        "      }",
        "    }",
        "  }",
        "}"
    );
    let result = read_contents(json);
    let open_api = result.openapi.clone().unwrap();
    assert_eq!(enum_property_default(&open_api, "SchemaWithDefaultAndEnum"), Some(s("SCHEMA_DEFAULT")));

    let options = resolve_fully_options();
    let result2 = OpenAPIV3Parser::new().read_contents(json, None, Some(&options));
    let open_api2 = result2.openapi.clone().unwrap();
    assert_eq!(enum_property_default(&open_api2, "SchemaWithDefaultAndEnum"), Some(s("SCHEMA_DEFAULT")));
}

#[test]
fn test_issue1454_all_of_enum_with_default_value() {
    let _g = common::setup(common::V3_MODULE);
    let json = concat!(
        "{",
        "  \"openapi\": \"3.0.1\",",
        "  \"info\": {",
        "    \"title\": \"Here be enum bugs\",",
        "    \"description\": \"OpenAPI spec to test the enum parsing bugs.\",",
        "    \"version\": \"1.0.0\"",
        "  },",
        "  \"paths\": {",
        "    \"/enum/parse/bug\": {",
        "      \"post\": {",
        "        \"summary\": \"Test enum parse bug\",",
        "        \"requestBody\": {",
        "          \"content\": {",
        "            \"application/json\": {",
        "              \"schema\": {",
        "                \"$ref\": \"#/components/schemas/SchemaWithEnumWithDefault\"",
        "              }",
        "            }",
        "          }",
        "        },",
        "        \"responses\": {",
        "          \"default\": {",
        "            \"description\": \"Successful response\"",
        "          }",
        "        }",
        "      }",
        "    }",
        "  },",
        "  \"components\": {",
        "    \"schemas\": {",
        "      \"SchemaWithEnumWithDefault\": {",
        "        \"type\": \"object\",",
        "        \"properties\": {",
        "          \"enumProperty\": {",
        "            \"description\": \"This schema with set default enum value and enum ref shows default SCHEMA_DEFAULT value in Swagger Editor, but is null when parsed with OpenAPIV3Parser.\",",
        "            \"allOf\": [",
        "              {",
        "                \"$ref\": \"#/components/schemas/EnumWithDefault\"",
        "              }",
        "            ]",
        "          }",
        "        }",
        "      },",
        "      \"EnumWithDefault\": {",
        "        \"type\": \"string\",",
        "        \"enum\": [",
        "          \"SCHEMA_DEFAULT\",",
        "          \"NOT_DEFAULT\"",
        "        ],",
        "        \"default\": \"SCHEMA_DEFAULT\"",
        "      }",
        "    }",
        "  }",
        "}"
    );
    // (the resolve=false variant is commented out upstream)
    let options = resolve_fully_options();
    let result2 = OpenAPIV3Parser::new().read_contents(json, None, Some(&options));
    let open_api2 = result2.openapi.clone().unwrap();
    assert_eq!(enum_property_default(&open_api2, "SchemaWithEnumWithDefault"), Some(s("SCHEMA_DEFAULT")));
}

#[test]
fn test_issue1454_with_default_all_of_enum_with_default_value() {
    let _g = common::setup(common::V3_MODULE);
    let json = concat!(
        "{",
        "  \"openapi\": \"3.0.1\",",
        "  \"info\": {",
        "    \"title\": \"Here be enum bugs\",",
        "    \"description\": \"OpenAPI spec to test the enum parsing bugs.\",",
        "    \"version\": \"1.0.0\"",
        "  },",
        "  \"paths\": {",
        "    \"/enum/parse/bug\": {",
        "      \"post\": {",
        "        \"summary\": \"Test enum parse bug\",",
        "        \"requestBody\": {",
        "          \"content\": {",
        "            \"application/json\": {",
        "              \"schema\": {",
        "                \"$ref\": \"#/components/schemas/SchemaWithDefaultAndEnumWithDefault\"",
        "              }",
        "            }",
        "          }",
        "        },",
        "        \"responses\": {",
        "          \"default\": {",
        "            \"description\": \"Successful response\"",
        "          }",
        "        }",
        "      }",
        "    }",
        "  },",
        "  \"components\": {",
        "    \"schemas\": {",
        "      \"SchemaWithDefaultAndEnumWithDefault\": {",
        "        \"type\": \"object\",",
        "        \"properties\": {",
        "          \"enumProperty\": {",
        "            \"description\": \"This schema with set default enum value and enum ref shows default SCHEMA_DEFAULT value in Swagger Editor, but is null when parsed with OpenAPIV3Parser.\",",
        "            \"allOf\": [",
        "              {",
        "                \"$ref\": \"#/components/schemas/EnumWithDefault\"",
        "              }",
        "            ],",
        "            \"default\": \"SCHEMA_DEFAULT\"",
        "          }",
        "        }",
        "      },",
        "      \"EnumWithDefault\": {",
        "        \"type\": \"string\",",
        "        \"enum\": [",
        "          \"SCHEMA_DEFAULT\",",
        "          \"NOT_DEFAULT\"",
        "        ],",
        "        \"default\": \"SCHEMA_DEFAULT\"",
        "      }",
        "    }",
        "  }",
        "}"
    );
    let result = OpenAPIV3Parser::new().read_contents(json, None, Some(&ParseOptions::default()));
    let open_api = result.openapi.clone().unwrap();
    assert_eq!(enum_property_default(&open_api, "SchemaWithDefaultAndEnumWithDefault"), Some(s("SCHEMA_DEFAULT")));

    let options = resolve_fully_options();
    let result2 = OpenAPIV3Parser::new().read_contents(json, None, Some(&options));
    let open_api2 = result2.openapi.clone().unwrap();
    assert_eq!(enum_property_default(&open_api2, "SchemaWithDefaultAndEnumWithDefault"), Some(s("SCHEMA_DEFAULT")));
}

#[test]
fn test_optional_parameter() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: 3.0.1\n",
        "paths:\n",
        "  \"/pet\":\n",
        "    summary: summary\n",
        "    description: description\n",
        "    post:\n",
        "      summary: Add a new pet to the store\n",
        "      description: ''\n",
        "      operationId: addPet\n",
        "      parameters:\n",
        "      - name: status\n",
        "        in: query\n",
        "        description: Status values that need to be considered for filter\n",
        "        schema:\n",
        "          type: array\n",
        "          items:\n",
        "            type: string\n",
        "            enum:\n",
        "            - available\n",
        "            - pending\n",
        "            - sold\n",
        "          default: available"
    );
    let result = read_contents_resolve(yaml);
    let open_api = result.openapi.clone().unwrap();
    let parameter = path(&open_api, "/pet").get_post().unwrap().get_parameters().unwrap().get(0).unwrap();
    // assertFalse(Boolean) unboxes: null would throw
    assert!(!parameter.get_required().expect("required"));
}

#[test]
fn test_discriminator_object() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: '3.0.1'\n",
        "components:\n",
        "  schemas:\n",
        "    Pet:\n",
        "      type: object\n",
        "      required:\n",
        "      - pet_type\n",
        "      properties:\n",
        "        pet_type:\n",
        "          type: string\n",
        "      discriminator:\n",
        "        propertyName: pet_type\n",
        "        mapping:\n",
        "          cachorro: Dog\n",
        "    Cat:\n",
        "      allOf:\n",
        "      - $ref: \"#/components/schemas/Pet\"\n",
        "      - type: object\n",
        "        # all other properties specific to a `Cat`\n",
        "        properties:\n",
        "          name:\n",
        "            type: string\n",
        "    Dog:\n",
        "      allOf:\n",
        "      - $ref: \"#/components/schemas/Pet\"\n",
        "      - type: object\n",
        "        # all other properties specific to a `Dog`\n",
        "        properties:\n",
        "          bark:\n",
        "            type: string\n",
        "    Lizard:\n",
        "      allOf:\n",
        "      - $ref: \"#/components/schemas/Pet\"\n",
        "      - type: object\n",
        "        # all other properties specific to a `Lizard`\n",
        "        properties:\n",
        "          lovesRocks:\n",
        "            type: boolean"
    );
    let result = read_contents_resolve(yaml);
    let messages = message_set(&result);

    let discriminator = schemas(&result.openapi.clone().unwrap()).get("Pet").unwrap().get_discriminator().unwrap();
    assert_eq!(discriminator.get_property_name().as_deref(), Some("pet_type"));
    assert_eq!(discriminator.get_mapping().unwrap().get("cachorro").as_deref(), Some("Dog"));
    assert!(messages.contains("attribute paths is missing"));
    assert!(messages.contains("attribute info is missing"));
}

#[test]
fn test_should_return_empty() {
    let _g = common::setup(common::V3_MODULE);
    let json = "{}";
    let result = read_contents_resolve(json);
    assert!(message_set(&result).contains("attribute openapi is missing"));
}

#[test]
fn test_almost_empty() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = concat!(
        "openapi: '3.0.1'\n",
        "new: extra"
    );
    let result = read_contents_resolve(yaml);
    let messages = message_set(&result);
    assert!(messages.contains("attribute info is missing"));
    assert!(messages.contains("attribute paths is missing"));
    assert!(messages.contains("attribute new is unexpected"));
}

#[test]
fn read_info_object() {
    let _g = common::setup(common::V3_MODULE);
    for root_node in data() {
        let result = deserialize(&root_node);

        let open_api = result.openapi.clone();
        assert!(open_api.is_some());
        let open_api = open_api.unwrap();
        assert_eq!(open_api.get_openapi().as_deref(), Some("3.0.1"));

        let info = open_api.get_info();
        assert!(info.is_some());
        let info = info.unwrap();
        assert_eq!(info.get_title().as_deref(), Some("Sample Pet Store App"));
        assert_eq!(info.get_description().as_deref(), Some("This is a sample server Petstore"));
        assert_eq!(info.get_terms_of_service().as_deref(), Some("http://swagger.io/terms/"));
        assert!(info.get_extensions().unwrap().get("x-info").is_some());
        assert_eq!(info.get_extensions().unwrap().get("x-info").unwrap().to_java_string(), "info extension");

        let contact = info.get_contact();
        assert!(contact.is_some());
        let contact = contact.unwrap();
        assert_eq!(contact.get_name().as_deref(), Some("API Support"));
        assert_eq!(contact.get_url().as_deref(), Some("http://www.example.com/support"));
        assert_eq!(contact.get_email().as_deref(), Some("support@example.com"));
        assert!(contact.get_extensions().unwrap().get("x-contact").is_some());
        assert_eq!(contact.get_extensions().unwrap().get("x-contact").unwrap().to_java_string(), "contact extension");

        let license = info.get_license();
        assert!(license.is_some());
        let license = license.unwrap();
        assert_eq!(license.get_name().as_deref(), Some("Apache 2.0"));
        assert_eq!(license.get_url().as_deref(), Some("http://www.apache.org/licenses/LICENSE-2.0.html"));
        assert!(license.get_extensions().is_some());

        assert_eq!(info.get_version().as_deref(), Some("1.0.1"));
    }
}

#[test]
fn read_server_object() {
    let _g = common::setup(common::V3_MODULE);
    for root_node in data() {
        let result = deserialize(&root_node);
        let open_api = result.openapi.clone();
        assert!(open_api.is_some());
        let open_api = open_api.unwrap();

        let server = open_api.get_servers();
        assert!(server.is_some());
        let server = server.unwrap();
        let s0 = server.get(0).unwrap();
        assert!(s0.get_url().is_some());
        assert_eq!(s0.get_url().as_deref(), Some("http://petstore.swagger.io/v2"));

        let s1 = server.get(1).unwrap();
        assert!(s1.get_url().is_some());
        assert!(s1.get_description().is_some());
        assert_eq!(s1.get_url().as_deref(), Some("https://development.gigantic-server.com/v1"));
        assert_eq!(s1.get_description().as_deref(), Some("Development server"));

        let s2 = server.get(2).unwrap();
        let vars = s2.get_variables();
        assert!(vars.is_some());
        let vars = vars.unwrap();
        let _ = vars.values();
        assert!(vars.get("username").is_some());
        assert_eq!(vars.get("username").unwrap().get_default().as_deref(), Some("demo"));
        assert_eq!(vars.get("username").unwrap().get_description().as_deref(), Some("this value is assigned by the service provider, in this example `gigantic-server.com`"));
        assert!(vars.get("port").unwrap().get_enum().is_some());
        assert_eq!(vars.get("port").unwrap().get_enum().unwrap().get(0).as_deref(), Some("8443"));
        assert_eq!(vars.get("port").unwrap().get_enum().unwrap().get(1).as_deref(), Some("443"));
        assert_eq!(vars.get("port").unwrap().get_default().as_deref(), Some("8443"));
        assert!(vars.get("port").is_some());
        assert!(vars.get("basePath").is_some());
        assert!(s2.get_extensions().unwrap().get("x-server").is_some());
        assert_eq!(s2.get_extensions().unwrap().get("x-server").unwrap().to_java_string(), "server extension");
        assert_eq!(vars.get("basePath").unwrap().get_description().as_deref(), Some("testing overwriting"));
        assert_eq!(vars.get("basePath").unwrap().get_default().as_deref(), Some("v2"));
    }
}

#[test]
fn read_missing_server_object() {
    let _g = common::setup(common::V3_MODULE);
    let root_node = read_yaml_resource("oas.yaml");
    let result = deserialize(&root_node);
    let open_api = result.openapi.clone();
    assert!(open_api.is_some());
    assert_eq!(open_api.unwrap().get_servers().unwrap().get(0).unwrap().get_url().as_deref(), Some("/"));
}

#[test]
fn read_empty_security_requirement() {
    let _g = common::setup(common::V3_MODULE);
    let root_node = read_yaml_resource("oas.yaml");
    let result = deserialize(&root_node);
    let open_api = result.openapi.clone();
    assert!(open_api.is_some());
    let open_api = open_api.unwrap();

    let security_requirement = open_api.get_security().unwrap().get(0).unwrap();
    assert!(security_requirement.is_empty());
    assert_eq!(open_api.get_security().unwrap().len(), 4);
}

#[test]
fn read_empty_server_object() {
    let _g = common::setup(common::V3_MODULE);
    let root_node = read_yaml_resource("oas2.yaml.template");
    let result = deserialize(&root_node);
    let open_api = result.openapi.clone();
    assert!(open_api.is_some());
    assert_eq!(open_api.unwrap().get_servers().unwrap().get(0).unwrap().get_url().as_deref(), Some("/"));
}

#[test]
fn read_content_object() {
    let _g = common::setup(common::V3_MODULE);
    for root_node in data() {
        let result = deserialize(&root_node);
        let open_api = result.openapi.clone();
        assert!(open_api.is_some());
        let open_api = open_api.unwrap();
        assert!(open_api.get_paths().is_some());

        let get = path(&open_api, "/pet/findByStatusContent").get_get();
        assert!(get.is_some());
        let get = get.unwrap();
        assert!(get.get_parameters().is_some());
        let params = get.get_parameters().unwrap();
        assert_eq!(params.len(), 3);

        let content0 = params.get(0).unwrap().get_content();
        assert!(content0.is_some());
        let content0 = content0.unwrap();
        assert_eq!(content0.len(), 1);
        let json = content0.get("application/json").unwrap();
        assert_eq!(json.get_schema().unwrap().get_type().as_deref(), Some("array"));
        assert!(json.get_example().is_none());
        assert_eq!(json.get_examples().unwrap().get("list").unwrap().get_summary().as_deref(), Some("List of Names"));
        assert_eq!(json.get_schema().unwrap().get_type().as_deref(), Some("array"));

        let content1 = params.get(1).unwrap().get_content();
        assert!(content1.is_some());
        let content1 = content1.unwrap();
        assert_eq!(content1.len(), 1);
        let xml = content1.get("application/xml").unwrap().get_examples().unwrap();
        assert_eq!(xml.get("list").unwrap().get_summary().as_deref(), Some("List of names"));
        assert_eq!(xml.get("list").unwrap().get_value(), Some(s("<Users><User name='Bob'/><User name='Diane'/><User name='Mary'/><User name='Bill'/></Users>")));
        assert!(xml.get("empty").unwrap().get_summary().is_some());
        assert_eq!(xml.get("empty").unwrap().get_summary().as_deref(), Some("Empty list"));
        assert_eq!(xml.get("empty").unwrap().get_value(), Some(s("<Users/>")));

        let content2 = params.get(2).unwrap().get_content();
        assert!(content2.is_some());
        let content2 = content2.unwrap();
        assert_eq!(content2.len(), 1);
        let text = content2.get("text/plain").unwrap().get_examples().unwrap();
        assert_eq!(text.get("list").unwrap().get_summary().as_deref(), Some("List of names"));
        assert_eq!(text.get("list").unwrap().get_value(), Some(s("Bob,Diane,Mary,Bill")));
        assert_eq!(text.get("empty").unwrap().get_summary().as_deref(), Some("Empty"));
        assert_eq!(text.get("empty").unwrap().get_value(), Some(s("")));

        let put = path(&open_api, "/pet").get_put();
        assert!(put.is_some());
        let response400 = put.unwrap().get_responses().unwrap().get("400").unwrap().get_content().unwrap();
        assert!(response400.get("application/json").is_some());
        assert_eq!(response400.len(), 1);
        assert_eq!(response400.get("application/json").unwrap().get_schema().unwrap().get_type().as_deref(), Some("array"));
    }
}

#[test]
fn read_request_body_object() {
    let _g = common::setup(common::V3_MODULE);
    for root_node in data() {
        let result = deserialize(&root_node);
        let open_api = result.openapi.clone();
        assert!(open_api.is_some());
        let open_api = open_api.unwrap();
        assert!(open_api.get_paths().is_some());

        let get = path(&open_api, "/pet/findByStatus").get_get().unwrap();
        assert!(get.get_request_body().is_some());
        let body = get.get_request_body().unwrap();
        assert_eq!(body.get_description().as_deref(), Some("pet store to add to the system"));
        assert!(body.get_required().unwrap(), "true");
        let mixed = body.get_content().unwrap().get("multipart/mixed");
        assert!(mixed.is_some());
        let mixed = mixed.unwrap();
        assert_eq!(mixed.get_schema().unwrap().get_type().as_deref(), Some("object"));
        assert!(mixed.get_schema().unwrap().get_properties().is_some());

        let encoding = mixed.get_encoding().unwrap();
        assert_eq!(encoding.get("historyMetadata").unwrap().get_content_type().as_deref(), Some("application/xml; charset=utf-8"));
        assert!(encoding.get("profileImage").unwrap().get_headers().is_some());
        assert!(encoding.get("profileImage").unwrap().get_headers().unwrap().get("X-Rate-Limit").is_some());
    }
}

#[test]
fn read_security_requirements_object() {
    let _g = common::setup(common::V3_MODULE);
    for root_node in data() {
        let result = deserialize(&root_node);
        let open_api = result.openapi.clone();
        assert!(open_api.is_some());
        let open_api = open_api.unwrap();

        let requirements = open_api.get_security();
        assert!(requirements.is_some());
        let requirements = requirements.unwrap();
        assert_eq!(requirements.len(), 2);

        let requirement = requirements.get(0).unwrap();
        assert!(requirement.contains_key("api_key"));

        let requirement = requirements.get(1).unwrap();
        assert!(requirement.contains_key("tokenAuth"));
    }
}

#[test]
fn read_security_schemes_object() {
    let _g = common::setup(common::V3_MODULE);
    for root_node in data() {
        let result = deserialize(&root_node);

        let messages = messages(&result);
        for m in [
            "attribute components.securitySchemes'.petstore_auth'.name is missing",
            "attribute components.securitySchemes'.petstore_auth'.in is missing",
            "attribute components.securitySchemes'.petstore_auth'.scheme is missing",
            "attribute components.securitySchemes'.petstore_auth'.openIdConnectUrl is missing",
            "attribute components.securitySchemes'.petstore_auth'.tokenUrl is missing",
            "attribute components.securitySchemes'.petstore_auth_password'.authorizationUrl is missing",
            "attribute components.securitySchemes'.petstore_auth_clientCredentials'.authorizationUrl is missing",
            "attribute components.securitySchemes'.api_key'.scheme is missing",
            "attribute components.securitySchemes'.api_key'.flows is missing",
            "attribute components.securitySchemes'.api_key'.openIdConnectUrl is missing",
            "attribute components.securitySchemes'.http'.name is missing",
            "attribute components.securitySchemes'.http'.in is missing",
            "attribute components.securitySchemes'.http'.flows is missing",
            "attribute components.securitySchemes'.http'.openIdConnectUrl is missing",
            "attribute components.securitySchemes'.openID'.name is missing",
            "attribute components.securitySchemes'.openID'.in is missing",
            "attribute components.securitySchemes'.openID'.scheme is missing",
            "attribute components.securitySchemes'.openID'.flows is missing",
        ] {
            assert!(!messages.contains(&m.to_string()), "{m}");
        }

        let open_api = result.openapi.clone();
        assert!(open_api.is_some());
        let open_api = open_api.unwrap();

        let security_schemes = open_api.get_components().unwrap().get_security_schemes();
        assert!(security_schemes.is_some());
        let security_schemes = security_schemes.unwrap();
        assert_eq!(security_schemes.len(), 10);

        let scheme = security_schemes.get("reference").unwrap();
        assert!(scheme.get_ref().unwrap() == "#/components/securitySchemes/api_key");

        let scheme = security_schemes.get("remote_reference").unwrap();
        assert!(scheme.get_ref().unwrap() == "http://localhost:${dynamicPort}/remote/security#/petstore_remote");

        for name in ["petstore_auth", "petstore_auth_password", "petstore_auth_clientCredentials", "petstore_auth_authorizationCode"] {
            let scheme = security_schemes.get(name).unwrap();
            assert!(scheme.get_type() == Some(SecuritySchemeType::Oauth2));
        }

        let scheme = security_schemes.get("api_key").unwrap();
        assert!(scheme.get_type() == Some(SecuritySchemeType::ApiKey));
        assert!(scheme.get_in() == Some(SecuritySchemeIn::Header));

        let scheme = security_schemes.get("api_key_cookie").unwrap();
        assert!(scheme.get_type() == Some(SecuritySchemeType::ApiKey));
        assert!(scheme.get_in() == Some(SecuritySchemeIn::Cookie));

        let scheme = security_schemes.get("http").unwrap();
        assert!(scheme.get_type() == Some(SecuritySchemeType::Http));

        let scheme = security_schemes.get("openID").unwrap();
        assert!(scheme.get_type() == Some(SecuritySchemeType::OpenIdConnect));
    }
}

#[test]
fn read_extensions() {
    let _g = common::setup(common::V3_MODULE);
    for root_node in data() {
        let result = deserialize(&root_node);
        let open_api = result.openapi.clone();
        assert!(open_api.is_some());
        let open_api = open_api.unwrap();
        assert!(open_api.get_extensions().is_some());
        let extensions = open_api.get_extensions().unwrap();
        assert!(extensions.contains_key("x-origin"));
        let object = extensions.get("x-origin").unwrap();

        let JavaObject::List(elements) = &object else { panic!("not a List: {object:?}") };
        assert_eq!(elements.len(), 1);
        let map = &elements[0];
        assert_eq!(map_get(map, "url"), s("http://petstore.swagger.io/v2/swagger.json"));
        assert_eq!(map_get(map, "format"), s("swagger"));
        assert_eq!(map_get(map, "version"), s("2.0"));

        let converter = map_get(map, "converter");
        assert!(!converter.is_null());
        assert_eq!(map_get(&converter, "url"), s("https://github.com/mermade/swagger2openapi"));
        assert_eq!(map_get(&converter, "version"), s("1.2.1"));

        let object = extensions.get("x-api-title").unwrap();
        assert!(matches!(object, JavaObject::String(_)));
        assert_eq!("pet store test api", object.to_java_string());
    }
}

#[test]
fn read_tag_object() {
    let _g = common::setup(common::V3_MODULE);
    for root_node in data() {
        let result = deserialize(&root_node);
        let open_api = result.openapi.clone();
        assert!(open_api.is_some());
        let open_api = open_api.unwrap();

        let tags = open_api.get_tags();
        assert!(tags.is_some());
        let tags = tags.unwrap();
        let t0 = tags.get(0).unwrap();
        assert!(t0.get_name().is_some());
        assert_eq!(t0.get_name().as_deref(), Some("pet"));
        assert!(t0.get_description().is_some());
        assert_eq!(t0.get_description().as_deref(), Some("Everything about your Pets"));
        assert!(t0.get_external_docs().is_some());

        let t1 = tags.get(1).unwrap();
        assert!(t1.get_name().is_some());
        assert!(t1.get_description().is_some());
        assert_eq!(t1.get_name().as_deref(), Some("store"));
        assert_eq!(t1.get_description().as_deref(), Some("Access to Petstore orders"));
    }
}

#[test]
fn read_examples_object() {
    let _g = common::setup(common::V3_MODULE);
    for root_node in data() {
        let result = deserialize(&root_node);
        let open_api = result.openapi.clone();
        assert!(open_api.is_some());
        let open_api = open_api.unwrap();

        assert!(open_api.get_paths().is_some());
        assert_eq!(paths(&open_api).len(), 19);

        //parameters operation get
        let get = path(&open_api, "/pet/findByStatus").get_get();
        assert!(get.is_some());
        let params = get.unwrap().get_parameters();
        assert!(params.is_some());
        let params = params.unwrap();
        assert_eq!(params.len(), 1);
        assert_eq!(params.get(0).unwrap().get_name().as_deref(), Some("status"));
        assert_eq!(params.get(0).unwrap().get_in().as_deref(), Some("query"));
    }
}

#[test]
fn read_schema_object() {
    let _g = common::setup(common::V3_MODULE);
    for root_node in data() {
        let result = deserialize(&root_node);
        let open_api = result.openapi.clone();
        assert!(open_api.is_some());
        let open_api = open_api.unwrap();

        assert!(open_api.get_paths().is_some());
        assert_eq!(paths(&open_api).len(), 19);

        //parameters operation get
        let get = path(&open_api, "/pet/findByStatus").get_get();
        assert!(get.is_some());
        let params = get.unwrap().get_parameters();
        assert!(params.is_some());
        let params = params.unwrap();
        assert_eq!(params.len(), 1);
        let p = params.get(0).unwrap();
        assert!(p.get_schema().is_some());
        assert_eq!(p.get_schema().unwrap().get_format().as_deref(), Some("int64"));
        assert_eq!(p.get_schema().unwrap().get_xml().unwrap().get_namespace().as_deref(), Some("http://example.com/schema/sample"));
        assert_eq!(p.get_schema().unwrap().get_xml().unwrap().get_prefix().as_deref(), Some("sample"));
        assert_eq!(p.get_in().as_deref(), Some("query"));
    }
}

#[test]
fn read_schema_array() {
    let _g = common::setup(common::V3_MODULE);
    for root_node in data() {
        let result = deserialize(&root_node);
        let open_api = result.openapi.clone();
        assert!(open_api.is_some());
        let open_api = open_api.unwrap();

        assert!(open_api.get_paths().is_some());
        assert_eq!(paths(&open_api).len(), 19);

        //parameters operation get
        let get = path(&open_api, "/pet/findByTags").get_get();
        assert!(get.is_some());
        let params = get.unwrap().get_parameters();
        assert!(params.is_some());
        let params = params.unwrap();
        assert_eq!(params.len(), 1);
        let p = params.get(0).unwrap();
        assert!(p.get_schema().is_some());
        assert_eq!(p.get_schema().unwrap().get_type().as_deref(), Some("array"));
        assert!(p.get_schema().unwrap().kind() == SchemaKind::Array, "(ArraySchema) cast");
        assert_eq!(p.get_schema().unwrap().get_items().unwrap().get_type().as_deref(), Some("string"));
        assert_eq!(p.get_name().as_deref(), Some("tags"));
        assert_eq!(p.get_explode(), Some(true));
        assert_eq!(p.get_style(), Some(ParameterStyle::Form));
    }
}

#[test]
fn read_produces_test_endpoint() {
    let _g = common::setup(common::V3_MODULE);
    for root_node in data() {
        let result = deserialize(&root_node);
        let open_api = result.openapi.clone();
        assert!(open_api.is_some());
        let open_api = open_api.unwrap();

        assert!(open_api.get_paths().is_some());
        assert_eq!(paths(&open_api).len(), 19);

        //parameters operation get
        let get = path(&open_api, "/producesTest").get_get();
        assert!(get.is_some());
        let operation = get.unwrap();
        assert!(operation.get_parameters().is_some());
        assert!(operation.get_parameters().unwrap().is_empty());

        let responses = operation.get_responses();
        assert!(responses.is_some());
        let responses = responses.unwrap();
        assert!(!responses.is_empty());

        let response = responses.get("200");
        assert!(response.is_some());
        let response = response.unwrap();
        assert_eq!(Some("it works"), response.get_description().as_deref());

        let content = response.get_content();
        assert!(content.is_some());
        let media_type = content.unwrap().get("application/json");
        assert!(media_type.is_some());

        let schema = media_type.unwrap().get_schema();
        assert!(schema.is_some());
        let schema = schema.unwrap();
        assert!(schema.kind() == SchemaKind::Object);

        let schema = schema.get_properties().unwrap().get("name");
        assert!(schema.is_some());
        assert!(schema.unwrap().kind() == SchemaKind::String);
    }
}

#[test]
fn read_external_docs_object() {
    let _g = common::setup(common::V3_MODULE);
    for root_node in data() {
        let result = deserialize(&root_node);
        let open_api = result.openapi.clone();
        assert!(open_api.is_some());
        let open_api = open_api.unwrap();

        let external_documentation = open_api.get_external_docs();
        assert!(external_documentation.is_some());
        let external_documentation = external_documentation.unwrap();
        assert!(external_documentation.get_url().is_some());
        assert_eq!(external_documentation.get_url().as_deref(), Some("http://swagger.io"));

        assert!(external_documentation.get_description().is_some());
        assert_eq!(external_documentation.get_description().as_deref(), Some("Find out more about Swagger"));
    }
}

#[test]
fn read_paths_object() {
    let _g = common::setup(common::V3_MODULE);
    for root_node in data() {
        let result = deserialize(&root_node);
        let open_api = result.openapi.clone();
        assert!(open_api.is_some());
        let open_api = open_api.unwrap();

        assert!(open_api.get_paths().is_some());
        assert_eq!(paths(&open_api).len(), 19);

        let _pet_ref = paths(&open_api).get("/pathItemRef");

        let pet_endpoint = paths(&open_api).get("/pet");
        assert!(pet_endpoint.is_some());
        let pet_endpoint = pet_endpoint.unwrap();
        assert_eq!(pet_endpoint.get_summary().as_deref(), Some("summary"));
        assert_eq!(pet_endpoint.get_description().as_deref(), Some("description"));
        let post = pet_endpoint.get_post().unwrap();
        assert!(post.get_external_docs().is_some());
        assert_eq!(post.get_external_docs().unwrap().get_url().as_deref(), Some("http://swagger.io"));
        assert_eq!(post.get_external_docs().unwrap().get_description().as_deref(), Some("Find out more"));

        //Operation trace
        assert!(pet_endpoint.get_trace().is_some());
        assert!(pet_endpoint.get_description().is_some());

        //Operation post
        assert!(post.get_tags().is_some());
        assert_eq!(post.get_tags().unwrap().len(), 1);
        assert_eq!(post.get_summary().as_deref(), Some("Add a new pet to the store"));
        assert_eq!(post.get_description().as_deref(), Some(""));
        assert_eq!(post.get_operation_id().as_deref(), Some("addPet"));
        assert!(pet_endpoint.get_servers().is_some());
        assert_eq!(pet_endpoint.get_servers().unwrap().len(), 1);
        assert!(pet_endpoint.get_parameters().is_some());
        assert_eq!(pet_endpoint.get_parameters().unwrap().len(), 2);
        assert!(post.get_parameters().is_some());
        let petstore_auth = post.get_security().unwrap().get(0).unwrap().get("petstore_auth").unwrap();
        assert_eq!(petstore_auth.get(0).as_deref(), Some("write:pets"));
        assert_eq!(petstore_auth.get(1).as_deref(), Some("read:pets"));

        let responses = post.get_responses();
        assert!(responses.is_some());
        let responses = responses.unwrap();
        assert!(responses.contains_key("405"));
        let response = responses.get("405").unwrap();
        assert_eq!(response.get_description().as_deref(), Some("Invalid input"));
        assert_eq!(response.get_headers().unwrap().get("X-Rate-Limit").unwrap().get_description().as_deref(), Some("calls per hour allowed by the user"));

        //parameters operation get
        let get = path(&open_api, "/pet/findByStatus").get_get();
        assert!(get.is_some());
        let get = get.unwrap();
        assert!(get.get_tags().is_some());
        assert_eq!(get.get_parameters().unwrap().len(), 1);
        assert_eq!(get.get_parameters().unwrap().get(0).unwrap().get_in().as_deref(), Some("query"));
        let hook = get.get_callbacks().unwrap().get("mainHook").unwrap().get("$request.body#/url").unwrap();
        assert_eq!(hook.get_post().unwrap().get_responses().unwrap().get("200").unwrap().get_description().as_deref(), Some("webhook successfully processed operation"));
    }
}

#[test]
fn read_components_object() {
    let _g = common::setup(common::V3_MODULE);
    for root_node in data() {
        let result = deserialize(&root_node);
        let open_api = result.openapi.clone();
        assert!(open_api.is_some());
        let open_api = open_api.unwrap();
        assert_eq!(open_api.get_openapi().as_deref(), Some("3.0.1"));

        let component = open_api.get_components();
        assert!(component.is_some());
        let component = component.unwrap();
        assert!(component.get_callbacks().is_some());
        let callbacks = component.get_callbacks().unwrap();
        let heartbeat = callbacks.get("heartbeat").unwrap().get("$request.query.heartbeat-url").unwrap();
        assert_eq!(heartbeat.get_post().unwrap().get_responses().unwrap().get("200").unwrap().get_description().as_deref(), Some("Consumer acknowledged the callback"));
        let failed = callbacks.get("failed").unwrap().get("$response.body#/failedUrl").unwrap();
        assert_eq!(failed.get_post().unwrap().get_responses().unwrap().get("200").unwrap().get_description().as_deref(), Some("Consumer acknowledged the callback failed"));

        assert!(component.get_examples().is_some());
        assert_eq!(component.get_examples().unwrap().get("cat").unwrap().get_summary().as_deref(), Some("An example of a cat"));
        assert!(component.get_examples().unwrap().get("cat").unwrap().get_value().is_some());

        assert!(component.get_headers().is_some());
        let header = component.get_headers().unwrap().get("X-Rate-Limit-Limit").unwrap();
        assert_eq!(header.get_description().as_deref(), Some("The number of allowed requests in the current period"));
        assert_eq!(header.get_schema().unwrap().get_type().as_deref(), Some("integer"));

        assert!(component.get_links().is_some());
        let link = component.get_links().unwrap().get("unsubscribe").unwrap();
        assert_eq!(link.get_operation_id().as_deref(), Some("cancelHookCallback"));
        assert!(link.get_parameters().is_some());
        assert_eq!(link.get_extensions().unwrap().get("x-link"), Some(s("link extension")));

        assert!(component.get_parameters().is_some());
        let skip = component.get_parameters().unwrap().get("skipParam").unwrap();
        assert_eq!(skip.get_name().as_deref(), Some("skip"));
        assert_eq!(skip.get_in().as_deref(), Some("query"));
        assert_eq!(skip.get_description().as_deref(), Some("number of items to skip"));
        assert!(skip.get_required().unwrap());
        assert_eq!(skip.get_schema().unwrap().get_type().as_deref(), Some("integer"));

        assert!(component.get_request_bodies().is_some());
        let bodies = component.get_request_bodies().unwrap();
        let body1 = bodies.get("requestBody1").unwrap();
        assert_eq!(body1.get_description().as_deref(), Some("request body in components"));
        assert_eq!(body1.get_content().unwrap().get("application/json").unwrap().get_schema().unwrap().get_ref().as_deref(), Some("#/components/schemas/Pet"));
        assert_eq!(body1.get_content().unwrap().get("application/xml").unwrap().get_schema().unwrap().get_ref().as_deref(), Some("#/components/schemas/Pet"));
        let body2_schema = bodies.get("requestBody2").unwrap().get_content().unwrap().get("application/json").unwrap().get_schema();
        assert_eq!(body2_schema.clone().unwrap().get_type().unwrap().to_string(), "array");
        assert!(body2_schema.is_some());

        assert!(component.get_responses().is_some());
        let responses = component.get_responses().unwrap();
        assert_eq!(responses.get("NotFound").unwrap().get_description().as_deref(), Some("Entity not found."));
        assert_eq!(responses.get("IllegalInput").unwrap().get_description().as_deref(), Some("Illegal input for operation."));
        assert_eq!(responses.get("GeneralError").unwrap().get_description().as_deref(), Some("General Error"));
        assert_eq!(responses.get("GeneralError").unwrap().get_content().unwrap().get("application/json").unwrap().get_schema().unwrap().get_ref().as_deref(), Some("#/components/schemas/ExtendedErrorModel"));

        assert!(component.get_schemas().is_some());
        let schemas = component.get_schemas().unwrap();
        assert_eq!(schemas.get("Category").unwrap().get_type().as_deref(), Some("object"));
        assert_eq!(schemas.get("ApiResponse").unwrap().get_required().unwrap().get(0).as_deref(), Some("name"));
        let order = schemas.get("Order").unwrap();
        assert_eq!(order.get_type().as_deref(), Some("object"));
        assert_eq!(order.get_not().unwrap().get_type().as_deref(), Some("integer"));
        assert!(matches!(order.get_additional_properties(), Some(AdditionalProperties::Schema(_))));
        let additional_properties = order.get_additional_properties().unwrap().as_schema().cloned().unwrap();
        assert_eq!(additional_properties.get_type().as_deref(), Some("integer"));

        let _schema = order.get_properties().unwrap().get("status");

        let properties = order.get_properties();
        assert!(properties.is_some());
        let properties = properties.unwrap();

        assert_eq!(properties.get("status").unwrap().get_type().as_deref(), Some("string"));
        assert_eq!(properties.get("status").unwrap().get_description().as_deref(), Some("Order Status"));
        assert_eq!(properties.get("status").unwrap().get_enum().unwrap().get(0), Some(s("placed")));

        assert!(component.get_security_schemes().is_some());
        let petstore_auth = component.get_security_schemes().unwrap().get("petstore_auth").unwrap();
        assert_eq!(petstore_auth.get_type().unwrap().to_string(), "oauth2");
        assert_eq!(petstore_auth.get_flows().unwrap().get_implicit().unwrap().get_authorization_url().as_deref(), Some("http://petstore.swagger.io/oauth/dialog"));
        assert!(petstore_auth.get_flows().unwrap().get_implicit().unwrap().get_scopes().is_some()); //TODO

        assert!(component.get_extensions().is_some());
        let extensions = component.get_extensions().unwrap();
        assert!(extensions.contains_key("x-component"));
        let object = extensions.get("x-component").unwrap();

        let JavaObject::List(elements) = &object else { panic!("not a List: {object:?}") };
        assert_eq!(elements.len(), 1);
        let map = &elements[0];
        assert_eq!(map_get(map, "url"), s("http://component.swagger.io/v2/swagger.json"));
        assert_eq!(map_get(map, "format"), s("OAS"));
        assert_eq!(map_get(map, "version"), s("3.0"));

        let converter = map_get(map, "converter");
        assert!(!converter.is_null());
        assert_eq!(map_get(&converter, "url"), s("https://github.com/mermade/oas3"));
        assert_eq!(map_get(&converter, "version"), s("1.2.3"));

        let object = extensions.get("x-api-title").unwrap();
        assert!(matches!(object, JavaObject::String(_)));
        assert_eq!("pet store test api in components", object.to_java_string());
    }
}

#[test]
fn read_oas() {
    let _g = common::setup(common::V3_MODULE);
    let root_node = read_yaml_resource("oas4.yaml");
    let result = deserialize(&root_node);
    let open_api = result.openapi.clone();
    assert!(open_api.is_some());
    let open_api = open_api.unwrap();

    assert!(open_api.get_paths().is_some());
    assert_eq!(paths(&open_api).len(), 114);

    let stripe = paths(&open_api).get("/v1/3d_secure");
    assert!(stripe.is_some());
    let stripe = stripe.unwrap();

    let post = stripe.get_post();
    assert!(post.is_some());
    let post = post.unwrap();
    assert_eq!(post.get_description().as_deref(), Some(""));
    assert_eq!(post.get_operation_id().as_deref(), Some("Create3DSecure"));
    assert!(post.get_parameters().is_some());

    let responses = post.get_responses();
    assert!(responses.is_some());
    let responses = responses.unwrap();
    assert!(responses.contains_key("200"));
    let response = responses.get("200").unwrap();
    assert_eq!(response.get_description().as_deref(), Some("Successful response."));
    assert_eq!(response.get_content().unwrap().get("application/json").unwrap().get_schema().unwrap().get_ref().as_deref(), Some("#/components/schemas/three_d_secure"));

    let stripe_get = paths(&open_api).get("/v1/account/external_accounts");
    assert!(stripe_get.is_some());
    let get = stripe_get.unwrap().get_get();
    assert!(get.is_some());
    let get = get.unwrap();
    assert_eq!(get.get_description().as_deref(), Some(""));
    assert_eq!(get.get_operation_id().as_deref(), Some("AllAccountExternalAccounts"));
    assert!(get.get_parameters().is_some());

    let responses_get = get.get_responses();
    assert!(responses_get.is_some());
    let responses_get = responses_get.unwrap();
    assert!(responses_get.contains_key("200"));
    let response_get = responses_get.get("200").unwrap();
    assert_eq!(response_get.get_description().as_deref(), Some("Successful response."));
    let properties = response_get.get_content().unwrap().get("application/json").unwrap().get_schema().unwrap().get_properties();

    assert!(properties.is_some());
    let properties = properties.unwrap();
    assert!(properties.get("data").unwrap().get_type().is_none());
    assert_eq!(properties.get("has_more").unwrap().get_description().as_deref(), Some("True if this list has another page of items after this one that can be fetched."));
    assert!(properties.get("data").unwrap().kind() == SchemaKind::Composed);

    let data = properties.get("data").unwrap();
    assert!(data.get_one_of().unwrap().get(0).unwrap().kind() == SchemaKind::Array);
    let items = data.get_one_of().unwrap().get(0).unwrap();
    assert_eq!(items.get_items().unwrap().get_ref().as_deref(), Some("#/components/schemas/bank_account"));
}

#[test]
fn test_issue1761() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { infer_schema_type: false, ..Default::default() };
    let result = OpenAPIV3Parser::new().read_location("./src/test/resources/issue-1761.yaml", None, Some(&options));

    let expected = concat!(
        "openapi: 3.0.3\n",
        "info:\n",
        "  title: openapi 3.0.3 sample spec\n",
        "  description: \"sample spec for testing openapi functionality, built from json schema\\\n",
        "    \\ tests for draft6\"\n",
        "  version: 0.0.1\n",
        "servers:\n",
        "- url: /\n",
        "paths: {}\n",
        "components:\n",
        "  schemas:\n",
        "    SimpleEnumValidation:\n",
        "      enum:\n",
        "      - 1\n",
        "      - 2\n",
        "      - 3\n",
        "    HeterogeneousEnumValidation:\n",
        "      enum:\n",
        "      - 6\n",
        "      - foo\n",
        "      - []\n",
        "      - true\n",
        "      - foo: 12\n",
        "    HeterogeneousEnumWithNullValidation:\n",
        "      enum:\n",
        "      - 6\n",
        "      - null\n",
        "    EnumsInProperties:\n",
        "      required:\n",
        "      - bar\n",
        "      type: object\n",
        "      properties:\n",
        "        foo:\n",
        "          enum:\n",
        "          - foo\n",
        "        bar:\n",
        "          enum:\n",
        "          - bar\n",
        "    EnumWithEscapedCharacters:\n",
        "      enum:\n",
        "      - |-\n",
        "        foo\n",
        "        bar\n",
        "      - \"foo\\rbar\"\n",
        "    EnumWithFalseDoesNotMatch0:\n",
        "      enum:\n",
        "      - false\n",
        "    EnumWithTrueDoesNotMatch1:\n",
        "      enum:\n",
        "      - true\n",
        "    EnumWith0DoesNotMatchFalse:\n",
        "      enum:\n",
        "      - 0\n",
        "    EnumWith1DoesNotMatchTrue:\n",
        "      enum:\n",
        "      - 1\n",
        "    NulCharactersInStrings:\n",
        "      enum:\n",
        "      - \"hello\\0there\"\n",
        "  x-schema-test-examples:\n",
        "    SimpleEnumValidation:\n",
        "      OneOfTheEnumIsValid:\n",
        "        description: one of the enum is valid\n",
        "        data: 1\n",
        "        valid: true\n",
        "      SomethingElseIsInvalid:\n",
        "        description: something else is invalid\n",
        "        data: 4\n",
        "        valid: false\n",
        "    HeterogeneousEnumValidation:\n",
        "      OneOfTheEnumIsValid:\n",
        "        description: one of the enum is valid\n",
        "        data: []\n",
        "        valid: true\n",
        "      SomethingElseIsInvalid:\n",
        "        description: something else is invalid\n",
        "        valid: false\n",
        "      ObjectsAreDeepCompared:\n",
        "        description: objects are deep compared\n",
        "        data:\n",
        "          foo: false\n",
        "        valid: false\n",
        "      ValidObjectMatches:\n",
        "        description: valid object matches\n",
        "        data:\n",
        "          foo: 12\n",
        "        valid: true\n",
        "      ExtraPropertiesInObjectIsInvalid:\n",
        "        description: extra properties in object is invalid\n",
        "        data:\n",
        "          foo: 12\n",
        "          boo: 42\n",
        "        valid: false\n",
        "    HeterogeneousEnumWithNullValidation:\n",
        "      NullIsValid:\n",
        "        description: null is valid\n",
        "        valid: true\n",
        "      NumberIsValid:\n",
        "        description: number is valid\n",
        "        data: 6\n",
        "        valid: true\n",
        "      SomethingElseIsInvalid:\n",
        "        description: something else is invalid\n",
        "        data: test\n",
        "        valid: false\n",
        "    EnumsInProperties:\n",
        "      BothPropertiesAreValid:\n",
        "        description: both properties are valid\n",
        "        data:\n",
        "          foo: foo\n",
        "          bar: bar\n",
        "        valid: true\n",
        "      WrongFooValue:\n",
        "        description: wrong foo value\n",
        "        data:\n",
        "          foo: foot\n",
        "          bar: bar\n",
        "        valid: false\n",
        "      WrongBarValue:\n",
        "        description: wrong bar value\n",
        "        data:\n",
        "          foo: foo\n",
        "          bar: bart\n",
        "        valid: false\n",
        "      MissingOptionalPropertyIsValid:\n",
        "        description: missing optional property is valid\n",
        "        data:\n",
        "          bar: bar\n",
        "        valid: true\n",
        "      MissingRequiredPropertyIsInvalid:\n",
        "        description: missing required property is invalid\n",
        "        data:\n",
        "          foo: foo\n",
        "        valid: false\n",
        "      MissingAllPropertiesIsInvalid:\n",
        "        description: missing all properties is invalid\n",
        "        data: {}\n",
        "        valid: false\n",
        "    EnumWithEscapedCharacters:\n",
        "      Member1IsValid:\n",
        "        description: member 1 is valid\n",
        "        data: |-\n",
        "          foo\n",
        "          bar\n",
        "        valid: true\n",
        "      Member2IsValid:\n",
        "        description: member 2 is valid\n",
        "        data: \"foo\\rbar\"\n",
        "        valid: true\n",
        "      AnotherStringIsInvalid:\n",
        "        description: another string is invalid\n",
        "        data: abc\n",
        "        valid: false\n",
        "    EnumWithFalseDoesNotMatch0:\n",
        "      FalseIsValid:\n",
        "        description: false is valid\n",
        "        data: false\n",
        "        valid: true\n",
        "      IntegerZeroIsInvalid:\n",
        "        description: integer zero is invalid\n",
        "        data: 0\n",
        "        valid: false\n",
        "      FloatZeroIsInvalid:\n",
        "        description: float zero is invalid\n",
        "        data: 0.0\n",
        "        valid: false\n",
        "    EnumWithTrueDoesNotMatch1:\n",
        "      TrueIsValid:\n",
        "        description: true is valid\n",
        "        data: true\n",
        "        valid: true\n",
        "      IntegerOneIsInvalid:\n",
        "        description: integer one is invalid\n",
        "        data: 1\n",
        "        valid: false\n",
        "      FloatOneIsInvalid:\n",
        "        description: float one is invalid\n",
        "        data: 1.0\n",
        "        valid: false\n",
        "    EnumWith0DoesNotMatchFalse:\n",
        "      FalseIsInvalid:\n",
        "        description: false is invalid\n",
        "        data: false\n",
        "        valid: false\n",
        "      IntegerZeroIsValid:\n",
        "        description: integer zero is valid\n",
        "        data: 0\n",
        "        valid: true\n",
        "      FloatZeroIsValid:\n",
        "        description: float zero is valid\n",
        "        data: 0.0\n",
        "        valid: true\n",
        "    EnumWith1DoesNotMatchTrue:\n",
        "      TrueIsInvalid:\n",
        "        description: true is invalid\n",
        "        data: true\n",
        "        valid: false\n",
        "      IntegerOneIsValid:\n",
        "        description: integer one is valid\n",
        "        data: 1\n",
        "        valid: true\n",
        "      FloatOneIsValid:\n",
        "        description: float one is valid\n",
        "        data: 1.0\n",
        "        valid: true\n",
        "    NulCharactersInStrings:\n",
        "      MatchStringWithNul:\n",
        "        description: match string with nul\n",
        "        data: \"hello\\0there\"\n",
        "        valid: true\n",
        "      DoNotMatchStringLackingNul:\n",
        "        description: do not match string lacking nul\n",
        "        data: hellothere\n",
        "        valid: false\n"
    );
    assert_eq!(yaml_pretty(&result.openapi.clone().unwrap()).unwrap(), expected);
}

#[test]
fn test_issue1572() {
    let _g = common::setup(common::V3_MODULE);
    let options = ParseOptions { infer_schema_type: false, ..Default::default() };
    let result = OpenAPIV3Parser::new().read_location("./src/test/resources/space in name/issue-1572.yaml", None, Some(&options));
    assert!(result.openapi.is_some());
}
