//! Port of `io.swagger.v3.parser.test.OpenAPIV3ParserTest`, the `@Test` methods declared from
//! `testFileReferenceWithVendorExt` (line 1782) to the end of the class.

mod common;

use jackson::JavaObject;
use std::rc::Rc;
use std::sync::Arc;
use swagger_parser::core::{AuthorizationValue, ParseOptions, SwaggerParseResult};
use swagger_parser::java::{JMap, JavaException};
use swagger_parser::models::{
    AdditionalProperties, ApiResponse, EncodingStyle, OpenAPI, Parameter, ParameterKind, Schema, SchemaKind, SecuritySchemeType, yaml_pretty,
    yaml_string,
};
use swagger_parser::parser::hooks::{self, Hooks};
use swagger_parser::parser::{OpenAPIResolver, OpenAPIV3Parser};

// ---------------------------------------------------------------------------------------------------------
// helpers

fn options(f: impl FnOnce(&mut ParseOptions)) -> ParseOptions {
    let mut o = ParseOptions::default();
    f(&mut o);
    o
}

fn schemas(o: &OpenAPI) -> JMap<Schema> {
    o.get_components().unwrap().get_schemas().unwrap()
}

fn schema(o: &OpenAPI, name: &str) -> Schema {
    schemas(o).get(name).unwrap_or_else(|| panic!("schema {name}"))
}

fn prop(s: &Schema, name: &str) -> Schema {
    s.get_properties().unwrap().get(name).unwrap_or_else(|| panic!("property {name}"))
}

fn items(s: &Schema) -> Schema {
    assert!(s.is_array_schema(), "(ArraySchema) cast of {:?}", s.kind());
    s.get_items().unwrap()
}

/// `(Schema) schema.getAdditionalProperties()`
fn additional(s: &Schema) -> Schema {
    match s.get_additional_properties() {
        Some(AdditionalProperties::Schema(s)) => s,
        other => panic!("additionalProperties is not a Schema: {other:?}"),
    }
}

fn messages(r: &SwaggerParseResult) -> Vec<String> {
    r.messages.clone().expect("getMessages() is null")
}

fn str_obj(s: &str) -> JavaObject {
    JavaObject::string(s)
}

/// `((Map) o).get(key)`
fn map_get(o: &JavaObject, key: &str) -> Option<JavaObject> {
    match o {
        JavaObject::Map(m) => m.get(key).cloned(),
        other => panic!("not a Map: {other:?}"),
    }
}

fn param(o: &OpenAPI, path: &str, i: usize) -> Parameter {
    o.get_paths().unwrap().get(path).unwrap().get_get().unwrap().get_parameters().unwrap().get(i).unwrap()
}

// ---------------------------------------------------------------------------------------------------------

#[test]
fn test_file_reference_with_vendor_ext() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPIV3Parser::new().read("src/test/resources/file-reference-with-vendor-ext/b.yaml").unwrap();
    let definitions = schemas(&open_api);
    let z = definitions.get("z").unwrap().get_extensions().unwrap().get("x-foo").unwrap();
    assert!(matches!(z, JavaObject::Map(_)));
    assert!(map_get(&z, "bar").unwrap().java_equals(&str_obj("baz")));
    let x = definitions.get("x").unwrap().get_extensions().unwrap().get("x-foo").unwrap();
    assert!(matches!(x, JavaObject::Map(_)));
    assert!(map_get(&x, "bar").unwrap().java_equals(&str_obj("baz")));
}

#[test]
fn test_troublesome_file() {
    let _g = common::setup(common::V3_MODULE);
    let _open_api = OpenAPIV3Parser::new().read("src/test/resources/troublesome.yaml");
}

/// `JsonToYamlFileDuplicator.duplicateFilesInYamlFormat`. The Java helper regenerates
/// `src/test/resources/relative-file-references/yaml`; `upstream/` is read-only here, so the copy goes to a
/// scratch directory under the target dir (given relative to the module directory, as upstream's path is).
fn duplicate_files_in_yaml_format(input_directory: &str, output_directory: &str) {
    use std::path::Path;
    let out = Path::new(output_directory);
    let _ = std::fs::remove_dir_all(out);
    std::fs::create_dir_all(out).unwrap();
    fn walk(dir: &Path, acc: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, acc);
            } else if p.extension().is_some_and(|x| x == "json") {
                acc.push(p);
            }
        }
    }
    let mut files = Vec::new();
    walk(Path::new(input_directory), &mut files);
    for next in files {
        let contents = common::read_file(next.to_str().unwrap()).replace(".json", ".yaml");
        let node = swagger_parser::parser::deserialization_utils::deserialize_into_tree_simple(&contents, next.to_str()).unwrap();
        // Yaml.mapper().writerWithDefaultPrettyPrinter().writeValueAsString(jsonNode)
        let mut w = jackson::yaml::YamlWriter::new(jackson::yaml::YamlGeneratorFeatures::swagger());
        jackson::convert::write_node(&mut w, &node, &jackson::convert::SerConfig::swagger()).unwrap();
        let yaml = w.into_string().unwrap();
        let relative = next.strip_prefix(input_directory).unwrap().with_extension("yaml");
        let output_file = out.join(relative);
        std::fs::create_dir_all(output_file.parent().unwrap()).unwrap();
        std::fs::write(output_file, yaml).unwrap();
    }
}

/// `env!("CARGO_TARGET_TMPDIR")` relative to the module directory (the working directory of the tests).
fn scratch_dir(name: &str) -> String {
    let root = env!("CARGO_MANIFEST_DIR");
    let tmp = env!("CARGO_TARGET_TMPDIR");
    match std::path::Path::new(tmp).strip_prefix(root) {
        Ok(rel) => format!("../../../{}/{name}", rel.display()),
        Err(_) => format!("{tmp}/{name}"),
    }
}

#[test]
fn test_load_relative_file_tree_yaml() {
    let _g = common::setup(common::V3_MODULE);
    let out = scratch_dir("relative-file-references/yaml");
    duplicate_files_in_yaml_format("src/test/resources/relative-file-references/json", &out);
    let open_api = do_relative_file_test(&format!("{out}/parent.yaml"));
    assert!(yaml_string(&open_api).is_ok());
}

#[test]
fn test_load_recursive_external_def() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPIV3Parser::new().read("src/test/resources/file-reference-to-recursive-defs/b.yaml").unwrap();
    let definitions = schemas(&open_api);
    assert_eq!(items(&prop(&definitions.get("v").unwrap(), "children")).get_ref().as_deref(), Some("#/components/schemas/v"));
    assert!(definitions.contains_key("y"));
    assert_eq!(items(&prop(&definitions.get("x").unwrap(), "children")).get_ref().as_deref(), Some("#/components/schemas/y"));
}

#[test]
fn test_load_nested_items_references() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| o.resolve = true);
    let result = OpenAPIV3Parser::new().read_location("src/test/resources/nested-items-references/b.yaml", None, Some(&o));
    let definitions = schemas(&result.openapi.unwrap());
    assert!(definitions.contains_key("z"));
    assert!(definitions.contains_key("w"));
}

#[test]
fn test_issue75() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPIV3Parser::new().read("src/test/resources/issue99.yaml").unwrap();
    let body = open_api.get_paths().unwrap().get("/albums").unwrap().get_post().unwrap().get_request_body().unwrap();
    let model = body.get_content().unwrap().get("application/json").unwrap().get_schema();
    let model = model.expect("model");
    assert!(model.is_array_schema());
    assert_eq!(model.get_items().unwrap().kind(), SchemaKind::ByteArray);
    assert_eq!(model.get_items().unwrap().get_format().as_deref(), Some("byte"));
}

/// `@Test(enabled = false)` upstream (it reads a document from raw.githubusercontent.com).
#[test]
#[ignore = "enabled = false upstream (see https://github.com/openAPI-api/openAPI-parser/issues/337)"]
fn test_issue62() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPIV3Parser::new()
        .read("https://raw.githubusercontent.com/OAI/OpenAPI-Specification/master/fixtures/v2.0/json/resources/resourceWithLinkedDefinitions.json")
        .unwrap();
    assert!(open_api.get_paths().unwrap().get("/pets/{petId}").unwrap().get_get().is_some());
}

#[test]
fn test_issue146() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPIV3Parser::new().read("src/test/resources/issue_146.yaml").expect("openAPI");
    let p = param(&open_api, "/checker", 0);
    assert_eq!(p.kind(), ParameterKind::Query);
    let pp = items(&p.get_schema().unwrap());
    assert_eq!(pp.kind(), SchemaKind::String);
    assert!("registration".eq_ignore_ascii_case(&pp.get_enum().unwrap().get(0).unwrap().to_java_string()));
}

#[test]
fn test_parameter_required() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPIV3Parser::new().read("src/test/resources/petstore.yaml").unwrap();
    let operation_params = open_api.get_paths().unwrap().get("/pet/{petId}").unwrap().get_post().unwrap().get_parameters().unwrap();
    let path_parameter = operation_params.get(0).unwrap();
    assert_eq!(path_parameter.kind(), ParameterKind::Path);
    assert_eq!(path_parameter.get_required(), Some(true));
}

#[test]
fn test_issue108() {
    let _g = common::setup(common::V3_MODULE);
    assert!(OpenAPIV3Parser::new().read("src/test/resources/issue_108.yaml").is_some());
}

#[test]
fn test_issue() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = "openapi: 3.0.0\n\
servers: []\n\
info:\n  description: No description provided.\n  version: '2.0'\n  title: My web service\n  x-endpoint-name: default\n\
paths:\n  x-nothing: sorry not supported\n  /foo:\n    x-something: 'yes, it is supported'\n    get:\n      responses:\n        '200':\n          description: OpenAPI API document for this service\n\
x-some-vendor:\n  sometesting: bye!";
    let result = OpenAPIV3Parser::new().read_contents(yaml, None, None);
    let open_api = result.openapi.unwrap();
    let vendor = open_api.get_extensions().unwrap().get("x-some-vendor").unwrap();
    assert!(map_get(&vendor, "sometesting").unwrap().java_equals(&str_obj("bye!")));
    let something = open_api.get_paths().unwrap().get("/foo").unwrap().get_extensions().unwrap().get("x-something").unwrap();
    assert!(something.java_equals(&str_obj("yes, it is supported")));
}

#[test]
fn test_issue292_with_csv_collection_format() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = "openapi: '3.0.0'\ninfo:\n  version: '0.0.0'\n  title: nada\npaths:\n  /persons:\n    get:\n      parameters:\n      - name: testParam\n        in: query\n        type: array\n        items:\n          type: string\n        explode: false\n      responses:\n        200:\n          description: Successful response";
    let result = OpenAPIV3Parser::new().read_contents(yaml, None, None);
    let open_api = result.openapi.unwrap();
    let qp = open_api.get_paths().unwrap().get("/persons").unwrap().get_get().unwrap().get_parameters().unwrap().get(0).unwrap();
    assert_eq!(qp.kind(), ParameterKind::Query);
    assert_eq!(qp.get_style().unwrap().to_string(), "form");
    assert_eq!(qp.get_explode(), Some(false));
}

#[test]
fn test_issue255() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPIV3Parser::new().read("objectExample.yaml").unwrap();
    assert_eq!(schema(&open_api, "SamplePayload").get_example().unwrap().to_java_string(), "[{\"op\":\"replace\",\"path\":\"/s\",\"v\":\"w\"}]");
}

#[test]
fn test_issue286() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPIV3Parser::new().read("issue_286.yaml").unwrap();
    let response = open_api.get_paths().unwrap().get("/").unwrap().get_get().unwrap().get_responses().unwrap().get("200").unwrap();
    let response = response.get_content().unwrap().get("*/*").unwrap().get_schema().unwrap();
    assert!(response.get_ref().is_some());
    assert_eq!(response.get_ref().as_deref(), Some("#/components/schemas/issue_286_PetList"));
    assert!(schemas(&open_api).get("issue_286_Allergy").is_some());
}

#[test]
fn test_issue360() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPIV3Parser::new().read("src/test/resources/issue_360.yaml").expect("openAPI");
    let paths = open_api.get_paths().unwrap();
    let body = paths.get("/pets").unwrap().get_post().unwrap().get_request_body().expect("body");
    let model = body.get_content().unwrap().get("application/json").unwrap().get_schema().expect("schema");
    assert!(model.get_properties().unwrap().get("foo").is_some());

    let extensions = body.get_extensions().expect("extensions");
    let o = extensions.get("x-examples").expect("x-examples");
    assert!(matches!(o, JavaObject::Map(_)));
    let jn = map_get(&o, "application/json").unwrap();
    assert!(matches!(jn, JavaObject::Map(_)));
    assert!(map_get(&jn, "foo").unwrap().java_equals(&str_obj("bar")));

    let string_body_parameter = paths.get("/otherPets").unwrap().get_post().unwrap().get_request_body().unwrap();
    assert_eq!(string_body_parameter.get_required(), Some(true));
    let sbp_model = string_body_parameter.get_content().unwrap().get("application/json").unwrap().get_schema().unwrap();
    assert_eq!(sbp_model.get_type().as_deref(), Some("string"));
    assert_eq!(sbp_model.get_format().as_deref(), Some("uuid"));
    let body_parameter = paths.get("/evenMorePets").unwrap().get_post().unwrap().get_request_body().unwrap();
    assert_eq!(body_parameter.get_required(), Some(true));
    let ref_model = body_parameter.get_content().unwrap().get("application/json").unwrap().get_schema().unwrap();
    assert!(ref_model.get_ref().is_some());
    assert_eq!(ref_model.get_ref().as_deref(), Some("#/components/schemas/Pet"));
}

#[test]
fn test_relative_path() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| {
        o.resolve = true;
        o.resolve_responses = true;
    });
    let read_result = OpenAPIV3Parser::new().read_location("src/test/resources/relative-issue/api.yaml", None, Some(&o));
    let open_api = read_result.openapi.unwrap();
    let _ = yaml_pretty(&open_api);
    let r = open_api.get_paths().unwrap().get("/scans").unwrap().get_get().unwrap().get_responses().unwrap().get("500").unwrap();
    assert_eq!(r.get_content().unwrap().get("application/json").unwrap().get_schema().unwrap().get_ref().as_deref(), Some("#/components/schemas/ErrorMessage"));
}

#[test]
fn test_relative_path2() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| {
        o.resolve = true;
        o.resolve_responses = true;
    });
    let read_result = OpenAPIV3Parser::new().read_location("src/test/resources/codegen-remote-responses/openapi.yaml", None, Some(&o));
    let open_api = read_result.openapi.unwrap();
    let r = open_api.get_paths().unwrap().get("/pet/findByTags").unwrap().get_get().unwrap().get_responses().unwrap().get("default").unwrap();
    assert_eq!(r.get_content().unwrap().get("application/json").unwrap().get_schema().unwrap().get_ref().as_deref(), Some("#/components/schemas/ErrorModel"));
}

#[test]
fn test_issue2105_equivalent_external_refs_use_single_component() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| o.resolve = true);
    let result = OpenAPIV3Parser::new().read_location("src/test/resources/oas3.fetched/openapi3.yaml", None, Some(&o));
    let open_api = result.openapi.unwrap();
    let s = schemas(&open_api);
    assert!(s.get("Event").is_some());
    assert!(s.get("EventList").is_some());
    assert!(s.get("Event_1").is_none());
    assert!(s.get("EventList_1").is_none());
}

fn do_relative_file_test(location: &str) -> OpenAPI {
    let o = options(|o| {
        o.resolve = true;
        o.resolve_responses = true;
    });
    let read_result = OpenAPIV3Parser::new().read_location(location, None, Some(&o));
    if !messages(&read_result).is_empty() {
        println!("{:?}", read_result.messages);
    }
    let open_api = read_result.openapi.unwrap();
    // `path.getClass() == PathItem.class`: PathItem has no subclasses here, so the lookup succeeding is the check
    let path = open_api.get_paths().unwrap().get("/health").unwrap();
    assert!(path.get_parameters().is_none());

    let operation = path.get_get().unwrap();
    let operation_params = operation.get_parameters().unwrap();
    assert_param_details(&operation_params.to_vec(), 0, ParameterKind::Query, "param1", "query");
    assert_param_details(&operation_params.to_vec(), 1, ParameterKind::Header, "param2", "header");
    assert_param_details(&operation_params.to_vec(), 2, ParameterKind::Path, "param3", "path");
    assert_param_details(&operation_params.to_vec(), 3, ParameterKind::Header, "param4", "header");

    let responses = operation.get_responses().unwrap();
    let get = |code: &str| responses.get(code).unwrap();
    assert_response(&open_api, &get("200"), "application/json", "Health information from the server", "#/components/schemas/health");
    assert_response(&open_api, &get("400"), "*/*", "Your request was not valid", "#/components/schemas/error");
    assert_response(&open_api, &get("500"), "*/*", "An unexpected error occur during processing", "#/components/schemas/error");

    let definitions = schemas(&open_api);
    let ref_in_definitions = definitions.get("refInDefinitions").unwrap();
    assert_eq!(ref_in_definitions.get_description().as_deref(), Some("The example model"));
    expected_properties_in_model(&ref_in_definitions, &["foo", "bar"]);

    let array_model = definitions.get("arrayModel").unwrap();
    assert_eq!(items(&array_model).get_ref().as_deref(), Some("#/components/schemas/foo"));

    let foo_model = definitions.get("foo").unwrap();
    assert_eq!(foo_model.get_description().as_deref(), Some("Just another model"));
    expected_properties_in_model(&foo_model, &["hello", "world"]);

    let composed_cat = definitions.get("composedCat").unwrap();
    assert!(composed_cat.is_composed_schema());
    let all_of = composed_cat.get_all_of().unwrap();
    let child = all_of.get(2).unwrap();
    expected_properties_in_model(&child, &["huntingSkill", "prop2", "reflexes", "reflexMap"]);
    let reflex_items = items(&prop(&child, "reflexes"));
    let r = reflex_items.get_ref().unwrap();
    assert_eq!(r, "#/components/schemas/reflex");
    assert!(definitions.contains_key(&r[r.rfind('/').unwrap() + 1..]));

    let reflex_map = prop(&child, "reflexMap");
    assert_eq!(additional(&reflex_map).get_ref().as_deref(), Some("#/components/schemas/reflex"));

    assert_eq!(all_of.len(), 3);
    assert_eq!(all_of.get(0).unwrap().get_ref().as_deref(), Some("#/components/schemas/pet"));
    assert_eq!(all_of.get(1).unwrap().get_ref().as_deref(), Some("#/components/schemas/foo_1"));
    open_api
}

fn expected_properties_in_model(model: &Schema, expected: &[&str]) {
    let props = model.get_properties().unwrap();
    assert_eq!(props.len(), expected.len());
    for p in expected {
        assert!(props.contains_key(p));
    }
}

fn assert_response(open_api: &OpenAPI, response: &ApiResponse, media_type: &str, expected_description: &str, expected_schema_ref: &str) {
    let schema = response.get_content().unwrap().get(media_type).unwrap().get_schema().unwrap();
    assert_eq!(response.get_description().as_deref(), Some(expected_description));
    assert_eq!(schema.kind(), SchemaKind::Schema);
    assert_eq!(schema.get_ref().as_deref(), Some(expected_schema_ref));
    let r = schema.get_ref().unwrap();
    assert!(schemas(open_api).contains_key(&r[r.rfind('/').unwrap() + 1..]));
}

fn assert_param_details(parameters: &[Parameter], index: usize, expected_type: ParameterKind, expected_name: &str, expected_in: &str) {
    let p = &parameters[index];
    assert_eq!(p.kind(), expected_type);
    assert_eq!(p.get_name().as_deref(), Some(expected_name));
    assert_eq!(p.get_in().as_deref(), Some(expected_in));
}

#[test]
fn test_nested_references() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPIV3Parser::new().read("src/test/resources/relative-file-references/json/parent.json").unwrap();
    let s = schemas(&open_api);
    for k in ["externalArray", "referencedByLocalArray", "externalObject", "referencedByLocalElement", "referencedBy"] {
        assert!(s.contains_key(k), "{k}");
    }
}

#[test]
fn test_codegen_petstore() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPIV3Parser::new().read("src/test/resources/petstore-codegen.yaml").unwrap();
    let enum_model = schemas(&open_api).get("Enum_Test").expect("Enum_Test");
    let enum_property = enum_model.get_properties().unwrap().get("enum_integer").expect("enum_integer");
    assert_eq!(enum_property.kind(), SchemaKind::Integer);
    let integers = enum_property.get_enum().unwrap();
    assert!(integers.get(0).unwrap().java_equals(&JavaObject::Integer(1)));
    assert!(integers.get(1).unwrap().java_equals(&JavaObject::Integer(-1)));

    let get_order_operation = open_api.get_paths().unwrap().get("/store/order/{orderId}").unwrap().get_get().expect("get");
    let order_id = get_order_operation.get_parameters().unwrap().get(0).unwrap();
    assert_eq!(order_id.kind(), ParameterKind::Path);
    let minimum = order_id.get_schema().unwrap().get_minimum().expect("minimum");
    assert_eq!(minimum.to_java_string(), "1");
}

#[test]
fn test_codegen_issue4555() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = "openapi: 3.0.0\ninfo:\n  title: test\n  version: \"0.0.1\"\n\npaths:\n  '/contents/{id}':\n    parameters:\n      - name: id\n        in: path\n        description: test\n        required: true\n        schema:\n          type: integer\n  get:\n    description: test\n    responses:\n      '200':\n        description: OK\n        schema: null\n        $ref: \"#/components/schemas/Content\"\ncomponents:\n  schemas:\n    Content:\n      type: object\n      title: \t\ttest";
    let result = OpenAPIV3Parser::new().read_contents(yaml, None, None);
    // can't parse with tabs!
    assert!(result.openapi.is_none());
}

#[test]
fn test_converter_issue17() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = "openapi: 3.0.0\ninfo:\n  version: 0.0.0\n  title: nada\npaths:\n  /persons:\n    get:\n      parameters:\n        - name: testParam\n          in: query\n          style: form\n          schema:\n            type: array\n            items:\n              type: string\n      responses:\n        '200':\n          description: Successful response\n          content:\n            '*/*':\n              schema:\n                $ref: \"#/components/schemas/Content\"\ncomponents:\n  schemas:\n    Content:\n      type: object";
    let o = options(|o| o.resolve = false);
    let result = OpenAPIV3Parser::new().read_contents(yaml, None, Some(&o));
    let open_api = result.openapi.expect("openAPI");
    let r = open_api.get_paths().unwrap().get("/persons").unwrap().get_get().unwrap().get_responses().unwrap().get("200").unwrap();
    assert_eq!(r.get_content().unwrap().get("*/*").unwrap().get_schema().unwrap().get_ref().as_deref(), Some("#/components/schemas/Content"));
}

#[test]
fn test_issue393() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = "openapi: 3.0.0\ninfo:\n  title: x\n  version: 1.0.0\npaths:\n  /test:\n    get:\n      responses:\n        '400':\n          description: >\n            The account could not be created because a credential didn't meet\n            the complexity requirements.\n          x-error-refs:\n            - $ref: \"#/x-error-defs/credentialTooShort\"\n            - $ref: \"#/x-error-defs/credentialTooLong\"\nx-error-defs:\n  credentialTooShort:\n    errorID: credentialTooShort";
    let result = OpenAPIV3Parser::new().read_contents(yaml, None, None);
    let open_api = result.openapi.expect("openAPI");
    assert!(open_api.get_extensions().unwrap().get("x-error-defs").is_some());
}

#[test]
fn test_bad_format() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPIV3Parser::new().read("src/test/resources/bad_format.yaml").unwrap();
    let path = open_api.get_paths().unwrap().get("/pets").unwrap();
    let params = path.get_get().unwrap().get_parameters().unwrap();

    for (i, name) in [(0, "query-param-int32"), (1, "query-param-invalid-format")] {
        let qp = params.get(i).expect("parameter");
        assert_eq!(qp.kind(), ParameterKind::Query);
        assert_eq!(qp.get_name().as_deref(), Some(name));
        let enum_values = qp.get_schema().unwrap().get_enum().expect("enum");
        assert_eq!(enum_values.len(), 3);
        assert!(enum_values.get(0).unwrap().java_equals(&JavaObject::Integer(1)));
        assert!(enum_values.get(1).unwrap().java_equals(&JavaObject::Integer(2)));
        assert!(enum_values.get(2).unwrap().java_equals(&JavaObject::Integer(7)));
    }

    let qp = params.get(2).expect("parameter");
    assert_eq!(qp.kind(), ParameterKind::Query);
    assert_eq!(qp.get_name().as_deref(), Some("query-param-collection-format-and-uniqueItems"));
    assert_eq!(qp.get_schema().unwrap().get_unique_items(), Some(true));
}

#[test]
fn test_issue357() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPIV3Parser::new().read("src/test/resources/issue_357.yaml").expect("openAPI");
    let get_params = open_api.get_paths().unwrap().get("/testApi").unwrap().get_get().unwrap().get_parameters().unwrap();
    assert_eq!(get_params.len(), 2);
    for p in get_params.to_vec() {
        match p.get_name().as_deref() {
            Some("pathParam1") => assert_eq!(p.get_schema().unwrap().get_type().as_deref(), Some("integer")),
            Some("pathParam2") => assert_eq!(p.get_schema().unwrap().get_type().as_deref(), Some("string")),
            other => panic!("Unexpected parameter named {other:?}"),
        }
    }
}

#[test]
fn test_issue358() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPIV3Parser::new().read("src/test/resources/issue_358.yaml").expect("openAPI");
    let parms = open_api.get_paths().unwrap().get("/testApi").unwrap().get_get().unwrap().get_parameters().unwrap();
    assert_eq!(parms.len(), 1);
    assert_eq!(parms.get(0).unwrap().get_name().as_deref(), Some("pathParam"));
    assert_eq!(parms.get(0).unwrap().get_schema().unwrap().get_type().as_deref(), Some("string"));
}

#[test]
fn test_incompatible_refs() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = "openapi: 3.0.0\nservers: []\npaths:\n  /test:\n    post:\n      responses:\n        '200':\n          $ref: \"#/components/schemas/Schema\"\n        '400':\n          definitions: this is right\n          description: Bad Request\n          content:\n            '*/*':\n              schema:\n                $ref: \"#/components/schemas/Schema\"\n      requestBody:\n        content:\n          application/json:\n            schema:\n              $ref: \"#/components/schemas/Schema\"\n        required: true\ninfo:\n  version: ''\n  title: ''\ncomponents:\n  schemas:\n    Schema: {}";
    let result = OpenAPIV3Parser::new().read_contents(yaml, None, None);
    assert!(result.openapi.is_some());
}

#[test]
fn test_issue243() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = "openapi: 3.0.0\nservers: []\ninfo:\n  version: 0.0.0\n  title: Simple API\npaths:\n  /:\n    get:\n      responses:\n        '200':\n          description: OK\n          content:\n            '*/*':\n              schema:\n                $ref: \"#/components/schemas/Simple\"\ncomponents:\n  schemas:\n    Simple:\n      type: string";
    let result = OpenAPIV3Parser::new().read_contents(yaml, None, None);
    assert!(result.openapi.is_some());
}

#[test]
fn test_issue594() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = "openapi: 3.0.0\nservers: []\npaths:\n  /test:\n    post:\n      responses:\n        '200':\n          description: OK\n      requestBody:\n        content:\n          application/json:\n            schema:\n              type: array\n              minItems: 1\n              maxItems: 1\n              items:\n                $ref: \"#/components/schemas/Pet\"\n        description: Hello world\ninfo:\n  version: ''\n  title: ''";
    let result = OpenAPIV3Parser::new().read_contents(yaml, None, None);
    let open_api = result.openapi.expect("openAPI");
    let schema = open_api.get_paths().unwrap().get("/test").unwrap().get_post().unwrap().get_request_body().unwrap();
    let schema = schema.get_content().unwrap().get("application/json").unwrap().get_schema().unwrap();
    assert_eq!(items(&schema).get_ref().as_deref(), Some("#/components/schemas/Pet"));
    assert!(schema.get_max_items().is_some());
    assert!(schema.get_min_items().is_some());
}

#[test]
fn test_issue450() {
    let _g = common::setup(common::V3_MODULE);
    let desc = "An array of Pets";
    let x_tag = "x-my-tag";
    let x_val = "An extension tag";
    let yaml = "openapi: 3.0.0\nservers: []\ninfo:\n  version: 0.0.0\n  title: Simple API\npaths:\n  /:\n    get:\n      responses:\n        '200':\n          description: OK\ncomponents:\n  schemas:\n    PetArray:\n      type: array\n      items:\n        $ref: \"#/components/schemas/Pet\"\n      description: An array of Pets\n      x-my-tag: An extension tag\n    Pet:\n      type: object\n      properties:\n        id:\n          type: string";
    let result = OpenAPIV3Parser::new().read_contents(yaml, None, None);
    let open_api = result.openapi.expect("openAPI");
    let pet_array = schemas(&open_api).get("PetArray").expect("PetArray");
    assert!(pet_array.is_array_schema());
    assert_eq!(pet_array.get_description().as_deref(), Some(desc));
    let ext = pet_array.get_extensions().expect("extensions");
    assert!(ext.get(x_tag).is_some());
    assert!(ext.get(x_tag).unwrap().java_equals(&str_obj(x_val)));
}

#[test]
fn test_issue480() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPIV3Parser::new().read("src/test/resources/issue-480.yaml").unwrap();
    let schemes = open_api.get_components().unwrap().get_security_schemes().unwrap();
    for key in schemes.keys() {
        let definition = schemes.get(&key).unwrap();
        if key == "petstore_auth" {
            assert_eq!(definition.get_type(), Some(SecuritySchemeType::Oauth2));
            assert_eq!(definition.get_description().as_deref(), Some("This is a description"));
        }
        if key == "api_key" {
            assert_eq!(definition.get_type(), Some(SecuritySchemeType::ApiKey));
            assert_eq!(definition.get_description().as_deref(), Some("This is another description"));
        }
    }
}

#[test]
fn check_all_of_are_taken() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPIV3Parser::new().read("src/test/resources/allOf-example/allOf.yaml").unwrap();
    assert_eq!(schemas(&open_api).len(), 2);
}

#[test]
fn check_path_parameter_required_value() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| o.resolve = true);
    let r = OpenAPIV3Parser::new().read_location("src/test/resources/issue-1319.yaml", None, Some(&o));
    assert_eq!(messages(&r).len(), 2);
    let open_api = r.openapi.unwrap();
    assert_eq!(schemas(&open_api).len(), 2);
    assert_eq!(open_api.get_paths().unwrap().len(), 2);
    assert_eq!(open_api.get_components().unwrap().get_parameters().unwrap().len(), 1);
}

/// Issue #616 Relative references inside of 'allOf'
#[test]
fn check_all_of_with_relative_references_are_found() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPIV3Parser::new().read("src/test/resources/allOf-relative-file-references/parent.yaml").unwrap();
    assert_eq!(schemas(&open_api).len(), 4);
}

/// Issue #616 Relative references inside of 'allOf'
#[test]
fn check_all_of_with_relative_references_issue604() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPIV3Parser::new().read("src/test/resources/allOf-relative-file-references/swagger.yaml").unwrap();
    assert_eq!(schemas(&open_api).len(), 2);
}

/// A string example should not be over quoted when parsing a yaml string
#[test]
fn reading_spec_string_should_not_over_quoting_string_example() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| o.resolve = false);
    let open_api = OpenAPIV3Parser::new().read_with(Some("src/test/resources/over-quoted-example.yaml"), None, Some(&o)).unwrap();
    assert!(schema(&open_api, "CustomerType").get_example().unwrap().java_equals(&str_obj("NoQuotePlease")));
}

/// A string example should not be over quoted when parsing a yaml node
#[test]
fn reading_spec_node_should_not_over_quoting_string_example() {
    let _g = common::setup(common::V3_MODULE);
    let yaml = common::read_file("src/test/resources/over-quoted-example.yaml");
    // Yaml.mapper().readValue(yaml, JsonNode.class)
    let root_node = jackson::yaml::read_tree(&yaml, jackson::yaml::YamlReadFeatures::swagger_core()).unwrap();
    let open_api = OpenAPIV3Parser::new().parse_json_node(None, &root_node).unwrap().openapi.unwrap();
    assert!(schema(&open_api, "CustomerType").get_example().unwrap().java_equals(&str_obj("NoQuotePlease")));
}

/// Issue 855: Request Body internal refs are not being resolved
#[test]
fn should_parse_request_body() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| o.resolve_fully = true);
    let open_api = OpenAPIV3Parser::new().read_with(Some("src/test/resources/issue_855.yaml"), None, Some(&o)).unwrap();
    let actual_component_content = open_api.get_components().unwrap().get_request_bodies().unwrap().get("ASinglePet").unwrap().get_content().unwrap();
    let actual_path_content = open_api.get_paths().unwrap().get("/adopt").unwrap().get_post().unwrap().get_request_body().unwrap().get_content();
    let properties = actual_component_content.get("application/petstore+json").unwrap().get_schema().unwrap().get_properties().expect("properties");
    assert_eq!(properties.len(), 2);
    let actual_path_content = actual_path_content.expect("path content");
    assert!(actual_path_content == actual_component_content);
}

#[test]
fn test_issue915() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| o.resolve_fully = true);
    let open_api = OpenAPIV3Parser::new().read_with(Some("src/test/resources/issue_918.yaml"), None, Some(&o)).unwrap();
    let headers = open_api.get_paths().unwrap().get("/2.0/users/").unwrap().get_get().unwrap().get_responses().unwrap().get("200").unwrap().get_headers().unwrap();
    assert_eq!(headers.get("X-Rate-Limit").unwrap().get_description().as_deref(), Some("The number of allowed requests in the current period"));
}

#[test]
fn test_issue931() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| {
        o.resolve = true;
        o.validate_internal_refs = false;
    });
    let result = OpenAPIV3Parser::new().read_location("Issue_931.json", None, Some(&o));
    assert!(result.openapi.is_some());
    let m = messages(&result);
    assert!(!m.is_empty());
    assert!(m[0].contains("doesn't adhere to regular expression ^[a-zA-Z0-9\\.\\-_]+$"));
}

#[test]
fn test_issue948() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| o.resolve = true);
    let result = OpenAPIV3Parser::new().read_location("Issue_948.json", None, Some(&o));
    OpenAPIResolver::new(result.openapi.as_ref()).unwrap().resolve().unwrap();
    assert!(result.openapi.is_some());
}

#[test]
fn should_parse_parameters() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| o.resolve_fully = true);
    let open_api = OpenAPIV3Parser::new().read_with(Some("src/test/resources/issue_877.yaml"), None, Some(&o)).unwrap();
    let p = open_api.get_paths().unwrap().get("/adopt").unwrap().get_get().unwrap().get_parameters().unwrap().get(0).expect("parameter");
    assert_eq!(p.get_in().as_deref(), Some("path"));
    assert_eq!(p.get_name().as_deref(), Some("playerId"));
}

#[test]
fn test_issue884() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| o.resolve_fully = true);
    let open_api = OpenAPIV3Parser::new().read_with(Some("src/test/resources/issue_884.yaml"), None, Some(&o)).unwrap();
    let links = open_api.get_paths().unwrap().get("/2.0/repositories/{username}").unwrap().get_get().unwrap().get_responses().unwrap().get("200").unwrap().get_links().unwrap();
    let user_repository = links.get("userRepository").unwrap();
    assert_eq!(user_repository.get_operation_id().as_deref(), Some("getRepository"));
    assert!(user_repository.get_headers().is_some());
}

#[test]
fn test_link_issue() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| o.resolve_fully = true);
    let open_api = OpenAPIV3Parser::new().read_with(Some("src/test/resources/linkIssue.yaml"), None, Some(&o)).unwrap();
    let links = open_api.get_paths().unwrap().get("/2.0/repositories/{username}").unwrap().get_get().unwrap().get_responses().unwrap().get("200").unwrap().get_links().unwrap();
    let request_body = links.get("userRepository").unwrap().get_request_body().unwrap();
    assert!(request_body.java_equals(&str_obj("$response.body#/slug")));
}

fn issue975_extract_properties_from_test_resource() -> JMap<Schema> {
    let o = options(|o| o.resolve = true);
    let open_api = OpenAPIV3Parser::new().read_location("issue-975/contract/openapi.yaml", None, Some(&o)).openapi.unwrap();
    schema(&open_api, "MyResponse").get_properties().unwrap()
}

const IMAGE: Option<&str> = Some("#/components/schemas/Image");

#[test]
fn test_issue975_property() {
    let _g = common::setup(common::V3_MODULE);
    let properties = issue975_extract_properties_from_test_resource();
    assert_eq!(properties.get("images").unwrap().get_ref().as_deref(), IMAGE);
}

#[test]
fn test_issue975_array() {
    let _g = common::setup(common::V3_MODULE);
    let properties = issue975_extract_properties_from_test_resource();
    assert_eq!(items(&properties.get("imagesArray").unwrap()).get_ref().as_deref(), IMAGE);
}

#[test]
fn test_issue975_map() {
    let _g = common::setup(common::V3_MODULE);
    let properties = issue975_extract_properties_from_test_resource();
    assert_eq!(additional(&properties.get("imagesMap").unwrap()).get_ref().as_deref(), IMAGE);
}

#[test]
fn test_issue975_array_map() {
    let _g = common::setup(common::V3_MODULE);
    let properties = issue975_extract_properties_from_test_resource();
    assert_eq!(additional(&items(&properties.get("imagesArrayMap").unwrap())).get_ref().as_deref(), IMAGE);
}

#[test]
fn test_issue975_map_array() {
    let _g = common::setup(common::V3_MODULE);
    let properties = issue975_extract_properties_from_test_resource();
    assert_eq!(items(&additional(&properties.get("imagesMapArray").unwrap())).get_ref().as_deref(), IMAGE);
}

#[test]
fn test_issue975_array_array() {
    let _g = common::setup(common::V3_MODULE);
    let properties = issue975_extract_properties_from_test_resource();
    assert_eq!(items(&items(&properties.get("imagesArrayArray").unwrap())).get_ref().as_deref(), IMAGE);
}

#[test]
fn test_issue975_map_map() {
    let _g = common::setup(common::V3_MODULE);
    let properties = issue975_extract_properties_from_test_resource();
    assert_eq!(additional(&additional(&properties.get("imagesMapMap").unwrap())).get_ref().as_deref(), IMAGE);
}

#[test]
fn test_issue975_one_of() {
    let _g = common::setup(common::V3_MODULE);
    let properties = issue975_extract_properties_from_test_resource();
    let composed = properties.get("oneOfExample").unwrap();
    assert!(composed.is_composed_schema());
    assert_eq!(composed.get_one_of().unwrap().get(0).unwrap().get_ref().as_deref(), IMAGE);
}

#[test]
fn test_issue975_any_of() {
    let _g = common::setup(common::V3_MODULE);
    let properties = issue975_extract_properties_from_test_resource();
    let composed = properties.get("anyOfExample").unwrap();
    assert!(composed.is_composed_schema());
    assert_eq!(composed.get_any_of().unwrap().get(0).unwrap().get_ref().as_deref(), IMAGE);
}

#[test]
fn test_issue975_all_of() {
    let _g = common::setup(common::V3_MODULE);
    let properties = issue975_extract_properties_from_test_resource();
    let composed = properties.get("allOfExample").unwrap();
    assert!(composed.is_composed_schema());
    assert_eq!(composed.get_all_of().unwrap().get(0).unwrap().get_ref().as_deref(), IMAGE);
}

#[test]
fn test_validation_issue() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| o.resolve_fully = true);
    let result = OpenAPIV3Parser::new().read_location("src/test/resources/validation/path-parameter-validation.yaml", None, Some(&o));
    assert!(messages(&result).is_empty());
}

#[test]
fn should_parse_external_schema_model_having_reference_to_its_local_model() {
    let _g = common::setup(common::V3_MODULE);
    let result = OpenAPIV3Parser::new().read("src/test/resources/issue-1040/api.yaml").unwrap();
    let model_schema = schema(&result, "Value");
    // `instanceof Schema` holds for every property value
    assert_eq!(prop(&model_schema, "id").get_ref().as_deref(), Some("#/components/schemas/ValueId"));
}

/// Test that extensions can be found on the class classloader in addition to tccl. There are no class
/// loaders here; the parser extensions are a fixed list, so reading succeeds regardless.
#[test]
fn test_issue1003_extensions_classloader() {
    let _g = common::setup(common::V3_MODULE);
    let api = OpenAPIV3Parser::new().read("src/test/resources/test.yaml");
    assert!(api.is_some());
}

#[test]
fn should_parse_api_with_multiple_parameter_references() {
    let _g = common::setup(common::V3_MODULE);
    let o = ParseOptions::default();
    let result = OpenAPIV3Parser::new().read_location("src/test/resources/issue-1063/api.yaml", Some(&[]), Some(&o));
    let api = result.openapi.clone().unwrap();
    let parameters = api.get_components().unwrap().get_parameters().unwrap();
    let mut keys = parameters.keys();
    keys.sort();
    assert_eq!(keys, vec!["IdParam".to_string(), "NameParam".to_string()]);
    assert_eq!(parameters.get("IdParam").unwrap().get_name().as_deref(), Some("id"));
    assert_eq!(parameters.get("NameParam").unwrap().get_name().as_deref(), Some("name"));
    assert_eq!(result.messages, Some(vec![]));
}

#[test]
fn should_parse_api_with_parameters_using_contentvs_schema() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| {
        o.resolve = true;
        o.resolve_fully = true;
    });
    let result = OpenAPIV3Parser::new().read_location("src/test/resources/issue-1078/api.yaml", Some(&[]), Some(&o));
    let api = result.openapi.unwrap();
    let components = api.get_components().expect("components");
    let s = components.get_schemas().unwrap();
    assert_eq!(s.len(), 4);
    for k in ["LocationType", "Lat", "Long", "SearchResult"] {
        assert!(s.get(k).is_some(), "{k}");
    }
    let parameters = api.get_paths().unwrap().get("/api-endpoint-1").unwrap().get_get().unwrap().get_parameters().unwrap();
    assert_eq!(parameters.len(), 1);
    let content = parameters.get(0).unwrap().get_content().expect("content");
    assert_eq!(content.len(), 1);
    let media_type = content.entries().into_iter().next().unwrap().1;
    let parameter_schema = media_type.get_schema().unwrap();
    assert!(parameter_schema.is_array_schema());
    let item_schema = parameter_schema.get_items().expect("items");
    assert!(item_schema.is_object_schema());
    let object_item_schemas = item_schema.get_properties().unwrap();
    assert_eq!(object_item_schemas.len(), 2);
    assert_eq!(object_item_schemas.get("lat").unwrap().kind(), SchemaKind::Integer);
    assert_eq!(object_item_schemas.get("long").unwrap().kind(), SchemaKind::Integer);
}

#[test]
fn test_issue1063() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| o.resolve = true);
    let result = OpenAPIV3Parser::new().read_location("src/test/resources/issue-1063/openapi.yaml", Some(&[]), Some(&o));
    let api = result.openapi.clone().unwrap();
    assert_eq!(param(&api, "/anPath", 0).get_name().as_deref(), Some("customer-id"));
    assert_eq!(param(&api, "/anPath", 1).get_name().as_deref(), Some("unit-id"));
    assert!(messages(&result).is_empty());
}

#[test]
fn test_issue1177() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| {
        o.resolve = true;
        o.resolve_fully = true;
        o.resolve_responses = true;
    });
    let auths: Vec<AuthorizationValue> = Vec::new();
    let open_api = OpenAPIV3Parser::new().read_location("/issue-1177/swagger.yaml", Some(&auths), Some(&o)).openapi.unwrap();

    // $ref response with $ref header
    let pets_list_api_response = open_api.get_paths().unwrap().get("/update-pets").unwrap().get_post().unwrap().get_responses().unwrap().get("200").expect("200");
    let session_id_header = pets_list_api_response.get_headers().unwrap().get("x-session-id").expect("header");
    let pets_list_schema = schemas(&open_api).get("PetsList").expect("PetsList");
    let headers = open_api.get_components().unwrap().get_headers().expect("headers");
    let session_id_header_component = headers.get("x-session-id").unwrap();
    assert!(session_id_header.ptr_eq(&session_id_header_component));
    assert!(pets_list_api_response.get_content().unwrap().get("application/json").unwrap().get_schema().unwrap().ptr_eq(&pets_list_schema));
}

#[test]
fn test_resolve_fully_map() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| o.resolve_fully = false);
    let open_api = OpenAPIV3Parser::new().read_location("resolve-fully-map.yaml", None, Some(&o)).openapi.unwrap();
    assert!(yaml_pretty(&open_api).unwrap().contains("$ref"));

    let o = options(|o| o.resolve_fully = true);
    let open_api = OpenAPIV3Parser::new().read_location("resolve-fully-map.yaml", None, Some(&o)).openapi.unwrap();
    assert!(!yaml_pretty(&open_api).unwrap().contains("$ref"));
}

#[test]
fn test_parse_options_skip_matches_false() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| {
        o.resolve = true;
        o.flatten = true;
        o.skip_matches = false;
    });
    let result = OpenAPIV3Parser::new().read_location("src/test/resources/skipMatches.yaml", None, Some(&o));
    let open_api = result.openapi.expect("openAPI");
    let s = open_api.get_components().expect("components").get_schemas().expect("schemas");
    assert_eq!(s.len(), 6);
}

#[test]
fn test_parse_options_skip_matches_true() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| {
        o.resolve = true;
        o.flatten = true;
        o.skip_matches = true;
    });
    let result = OpenAPIV3Parser::new().read_location("src/test/resources/skipMatches.yaml", None, Some(&o));
    let open_api = result.openapi.expect("openAPI");
    let s = open_api.get_components().expect("components").get_schemas().expect("schemas");
    assert_eq!(s.len(), 6);
}

#[test]
fn test_issue1335() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| o.resolve = true);
    let result = OpenAPIV3Parser::new().read_location("src/test/resources/issue1335.yaml", None, Some(&o));
    assert!(result.openapi.unwrap().get_components().unwrap().get_examples().unwrap().get("ex1").is_some());
}

#[test]
fn test_empty_query_parameter_example() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| o.resolve = true);
    let result = OpenAPIV3Parser::new().read_location("src/test/resources/emptyQueryParameter.yaml", None, Some(&o));
    assert!(param(&result.openapi.unwrap(), "/foo", 0).get_example().unwrap().java_equals(&str_obj("")));
}

#[test]
fn test_blank_query_parameter_example() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| o.resolve = true);
    let result = OpenAPIV3Parser::new().read_location("src/test/resources/blankQueryParameter.yaml", None, Some(&o));
    assert!(param(&result.openapi.unwrap(), "/foo", 0).get_example().unwrap().java_equals(&str_obj(" ")));
}

#[test]
fn test_regression_issue1236() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| o.resolve = true);
    let result = OpenAPIV3Parser::new().read_location("src/test/resources/testRegressionIssue1236.yaml", None, Some(&o));
    assert_eq!(messages(&result).len(), 0);
}

#[test]
fn test_sample_parser() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| o.resolve = true);
    let result = OpenAPIV3Parser::new().read_location("src/test/resources/issue-1211.json", None, Some(&o));
    println!("{:?}", result.messages);
    assert!(result.openapi.is_some());
    let m = messages(&result);
    assert!(!m.is_empty());
    assert!(m[0].contains("components.schemas.Pet. writeOnly and readOnly are both present"));
}

#[test]
fn test_duplicate_http_status_codes_json() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| o.resolve = true);
    let result = OpenAPIV3Parser::new().read_location("src/test/resources/duplicateHttpStatusCodes.json", None, Some(&o));
    let m = messages(&result);
    assert_eq!(m.len(), 1);
    assert_eq!(m[0], "Duplicate field '200' in `src/test/resources/duplicateHttpStatusCodes.json`");
}

#[test]
fn test_duplicate_http_status_codes_yaml() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| o.resolve = true);
    let result = OpenAPIV3Parser::new().read_location("src/test/resources/duplicateHttpStatusCodes.yaml", None, Some(&o));
    let m = messages(&result);
    assert_eq!(m.len(), 1);
    assert_eq!(m[0], "Duplicate field 200 in `src/test/resources/duplicateHttpStatusCodes.yaml`");
}

fn discriminator_cat(location: &str) -> Option<Schema> {
    let open_api = OpenAPIV3Parser::new().read(location).expect("openAPI");
    schemas(&open_api).get("Cat")
}

#[test]
fn test_discriminator_single_file_no_mapping() {
    let _g = common::setup(common::V3_MODULE);
    assert!(discriminator_cat("./discriminator-mapping-resolution/single-file-no-mapping.yaml").is_some());
}

#[test]
fn test_discriminator_separate_file_no_mapping() {
    let _g = common::setup(common::V3_MODULE);
    // FIXME, issue #970 still exists in this form
    assert!(discriminator_cat("./discriminator-mapping-resolution/main-no-mapping.yaml").is_none());
}

#[test]
fn test_discriminator_single_file_plain_mapping() {
    let _g = common::setup(common::V3_MODULE);
    assert!(discriminator_cat("./discriminator-mapping-resolution/single-file-plain-mapping.yaml").is_some());
}

#[test]
fn test_discriminator_separate_file_plain_mapping() {
    let _g = common::setup(common::V3_MODULE);
    assert!(discriminator_cat("./discriminator-mapping-resolution/main-plain-mapping.yaml").is_some());
}

#[test]
fn test_discriminator_single_file_internal_mapping() {
    let _g = common::setup(common::V3_MODULE);
    assert!(discriminator_cat("./discriminator-mapping-resolution/single-file-internal-mapping.yaml").is_some());
}

#[test]
fn test_discriminator_separate_file_internal_mapping() {
    let _g = common::setup(common::V3_MODULE);
    assert!(discriminator_cat("./discriminator-mapping-resolution/main-internal-mapping.yaml").is_some());
}

#[test]
fn test_discriminator_same_file_external_mapping() {
    let _g = common::setup(common::V3_MODULE);
    assert!(discriminator_cat("./discriminator-mapping-resolution/main-external-mapping.yaml").is_some());
}

#[test]
fn test_discriminator_separate_file_external_mapping() {
    let _g = common::setup(common::V3_MODULE);
    assert!(discriminator_cat("./discriminator-mapping-resolution/main-external-mapping-3files.yaml").is_some());
}

/// Relative external refs failed on windows due to \ issue
#[test]
fn test_relative_path_issue1543() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| {
        o.resolve_fully = true;
        o.resolve = true;
    });
    let read_result = OpenAPIV3Parser::new().read_location("src/test/resources/issue-1543/openapi30.json", None, Some(&o));
    let m = messages(&read_result);
    if !m.is_empty() {
        assert!(!m[0].contains("end -1"));
    }
}

#[test]
fn test_issue1540() {
    let _g = common::setup(common::V3_MODULE);
    let open_api = OpenAPIV3Parser::new().read("./issue-1540/a.json").expect("openAPI");
    let extensions = open_api.get_extensions().expect("extensions");
    assert!(!extensions.is_empty());
    assert!(extensions.get("x-setting").is_some());
    let test_put_extensions = open_api.get_paths().unwrap().get("/test").unwrap().get_put().unwrap().get_extensions().expect("extensions");
    assert!(!test_put_extensions.is_empty());
    let x_order = test_put_extensions.get("x-order").expect("x-order");
    // (String) cast
    assert_eq!(x_order.as_str(), Some("2147483647"));
}

#[test]
fn test_issue1592() {
    let _g = common::setup(common::V3_MODULE);
    // new URL("jar:" + new File("src/test/resources/issue-1592.jar").toURI() + "!/test.yaml")
    let file = std::env::current_dir().unwrap().join("src/test/resources/issue-1592.jar");
    let url = format!("jar:file:{}!/test.yaml", file.display());
    let o = options(|o| o.resolve = true);
    let result = OpenAPIV3Parser::new().read_location(&url, None, Some(&o));
    assert!(result.openapi.is_some());
}

#[test]
fn test_issue1266() {
    let _g = common::setup(common::V3_MODULE);
    let yaml_string = common::read_file("src/test/resources/issue-1266/issue-1266.yaml");
    let yaml_string_resolved = common::read_file("src/test/resources/issue-1266/issue-1266-resolved.yaml");
    let o = options(|o| {
        o.resolve_fully = true;
        o.resolve_combinators = true;
    });
    let open_api = OpenAPIV3Parser::new().read_contents(&yaml_string, None, Some(&o)).openapi.expect("openAPI");
    assert_eq!(yaml_pretty(&open_api).unwrap(), yaml_string_resolved);
}

/// option true, adds Original Location to messages when ref is relative/local
#[test]
fn test_validate_external_refs_true() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| {
        o.validate_external_refs = true;
        o.resolve = true;
    });
    let result = OpenAPIV3Parser::new().read_location("./swos-443/root.yaml", None, Some(&o));
    assert!(result.openapi.is_some());
    let m = messages(&result);
    assert_eq!(m.len(), 20, "{m:#?}");
    for expected in [
        "attribute components.requestBodies.NewItem.asdasd is unexpected (./ref.yaml)",
        "attribute components.requestBodies.NewItem.descasdasdription is unexpected (./ref.yaml)",
        "attribute components.responses.GeneralError.descrsaiption is unexpected (./ref.yaml)",
        "attribute components.responses.GeneralError.asdas is unexpected (./ref.yaml)",
        "attribute components.responses.GeneralError.description is missing (./ref.yaml)",
        "attribute components.schemas.Examples.nonExpected is unexpected (./ref.yaml)",
        "attribute components.parameters.skipParam.[skip].in is not of type `[query|header|path|cookie]` (./ref.yaml)",
        "attribute components.securitySchemes.api_key.namex is unexpected (./ref.yaml)",
        "attribute components.securitySchemes.api_key.name is missing (./ref.yaml)",
        "attribute components.securitySchemes.api_key.in is not of type `cookie|header|query` (./ref.yaml)",
        "attribute components.callbacks.webhookVerificationEvent.postx is unexpected (./ref.yaml)",
        "attribute components.headers.X-Rate-Limit-Limit.descriptasdd is unexpected (./ref.yaml)",
        "attribute components.links.unsubscribe.parametersx is unexpected (./ref.yaml)",
        "attribute components.examples.response-example.summaryx is unexpected (./ref.yaml)",
        "components.examples.response-example. value and externalValue are both present (./ref.yaml)",
        "attribute components.callbacks.failed.wrongField is not of type `object` (./ref.yaml)",
        "attribute paths.~1refPet(get).responses is missing (./ref.yaml)",
        // error message in main file
        "attribute components.schemas.InvalidSchema.invalid is unexpected",
        "attribute components.schemas.ErrorModel.properties is not of type `object` (./ref.yaml)",
        "attribute components.schemas.Examples.properties is not of type `object` (./ref.yaml)",
    ] {
        assert!(m.iter().any(|x| x == expected), "missing {expected}");
    }
}

/// test that a model in a folder that has a ref to a model in the classpath is properly resolved.
#[test]
fn test_issue1891() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| {
        o.resolve = true;
        o.flatten = true;
    });
    let result = OpenAPIV3Parser::new().read_location("./issue-1891/openapi.yaml", None, Some(&o));
    assert!(messages(&result).is_empty());
    let open_api = result.openapi.unwrap();
    let local_model = schema(&open_api, "LocalModel");
    let types_model = schema(&open_api, "TypesModel");
    let shared_model = schemas(&open_api).get("SharedModel");
    assert_eq!(prop(&local_model, "sharedModelField").get_ref().as_deref(), Some("#/components/schemas/TypesModel"));
    assert_eq!(types_model.get_ref().as_deref(), Some("#/components/schemas/SharedModel"));
    assert!(shared_model.is_some());
}

/// directly parsed  definition, tested in previous method as reference relative/local
#[test]
fn test_validate_definition() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| o.validate_external_refs = false);
    let result = OpenAPIV3Parser::new().read_location("./swos-443/ref.yaml", None, Some(&o));
    assert!(result.openapi.is_some());
    let m = messages(&result);
    assert!(m.iter().any(|x| x == "attribute components.schemas.ErrorModel.properties is not of type `object`"));
    assert!(m.iter().any(|x| x == "attribute components.schemas.Examples.properties is not of type `object`"));
}

/// option false, does not add Original Location to messages when ref is relative/local
#[test]
fn test_validate_external_refs_false() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| {
        o.resolve = true;
        o.validate_external_refs = false;
    });
    let result = OpenAPIV3Parser::new().read_location("./swos-443/root.yaml", None, Some(&o));
    assert!(result.openapi.is_some());
    // keeps error messages only from original spec
    let m = messages(&result);
    assert!(m.iter().any(|x| x == "attribute components.schemas.InvalidSchema.invalid is unexpected"), "{m:#?}");
    assert!(
        m.iter().any(|x| x == "An exception was thrown while trying to deserialize the contents of ./ref.yaml into type class io.swagger.v3.oas.models.callbacks.Callback"),
        "{m:#?}"
    );
}

#[test]
fn test_null_example() {
    let _g = common::setup(common::V3_MODULE);
    let yaml_string = common::read_file("src/test/resources/null-full-example.yaml");
    let yaml_string_resolved = common::read_file("src/test/resources/null-full-example-resolved.yaml");
    let o = options(|o| {
        o.resolve_fully = true;
        o.resolve_combinators = false;
    });
    let open_api = OpenAPIV3Parser::new().read_contents(&yaml_string, None, Some(&o)).openapi.expect("openAPI");
    assert_eq!(yaml_pretty(&open_api).unwrap(), yaml_string_resolved);
}

#[test]
fn test_internal_refs_validation() {
    let _g = common::setup(common::V3_MODULE);
    let yaml_string = common::read_file("src/test/resources/internal-refs.yaml");
    let o = ParseOptions::default();
    let parse_result = OpenAPIV3Parser::new().read_contents(&yaml_string, None, Some(&o));
    assert_eq!(messages(&parse_result).len(), 1);
}

#[test]
fn test_issue1902_component_example_not_parse_ordinal_json() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| o.resolve = true);
    let result = OpenAPIV3Parser::new().read_location("src/test/resources/issue1902/1902-string-example-in-component.yaml", None, Some(&o));
    let open_api = result.openapi.expect("openAPI");
    assert!(open_api.get_components().unwrap().get_examples().unwrap().get("Things").is_some());
}

#[test]
fn test_issue_1746_headers_relative_paths() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| o.resolve = true);
    let parse_result = OpenAPIV3Parser::new().read_location("issue-1746/petstore.yml", None, Some(&o));
    let open_api = parse_result.openapi.unwrap();
    let r = open_api.get_paths().unwrap().get("/pets").unwrap().get_get().unwrap().get_responses().unwrap().get("200").unwrap();
    assert_eq!(r.get_headers().unwrap().get("x-next").unwrap().get_ref().as_deref(), Some("#/components/headers/LocationInHeaders"));
}

// ---------------------------------------------------------------------------------------------------------
// Safe URL resolving. Upstream these tests fetch https://petstore3.swagger.io/api/v3/openapi.json and
// https://petstore.swagger.io/v2/swagger.json; both answer (Java's result then has no messages, so the
// message loops pass vacuously), and the loops accept the messages of a 403 or of a denied URL. The suite
// must not need the network: host names resolve through a fixed table and the two documents are served
// from local petstore fixtures (`petstore.yaml`, the Swagger 2.0 `swagger.json`).

fn offline_remote() -> hooks::HooksGuard {
    let net: Arc<dyn swagger_parser_safe_url_resolver::NetUtils> = Arc::new(common::FixedNetUtils::default());
    hooks::install(Hooks {
        url_to_string: Some(Rc::new(|url: &str, _| {
            Some(match url {
                "https://petstore3.swagger.io/api/v3/openapi.json" => Ok(common::read_file("src/test/resources/petstore.yaml")),
                "https://petstore.swagger.io/v2/swagger.json" => Ok(common::read_file("src/test/resources/swagger.json")),
                other => Err(JavaException::new("java.net.UnknownHostException", other.to_string())),
            })
        })),
        net_utils: Some(net),
        ..Default::default()
    })
}

/// Test safe resolving
#[test]
fn test31_safe_url_resolving() {
    let _g = common::setup(common::V3_MODULE);
    let _h = offline_remote();
    let o = options(|o| {
        o.resolve_fully = true;
        o.safely_resolve_url = true;
        o.remote_ref_allow_list = Some(vec![]);
        o.remote_ref_block_list = Some(vec![]);
    });
    let result = OpenAPIV3Parser::new().read_location("safeResolving/oas30SafeUrlResolvingWithPetstore.yaml", None, Some(&o));
    if let Some(m) = &result.messages {
        for message in m {
            assert!(message.contains("Server returned HTTP response code: 403"), "{message}");
        }
    }
}

/// Test safe resolving with blocked URL
#[test]
fn test31_safe_url_resolving_with_blocked_url() {
    let _g = common::setup(common::V3_MODULE);
    let _h = offline_remote();
    let o = options(|o| {
        o.resolve_fully = true;
        o.safely_resolve_url = true;
        o.remote_ref_allow_list = Some(vec![]);
        o.remote_ref_block_list = Some(vec!["petstore3.swagger.io".into()]);
    });
    let result = OpenAPIV3Parser::new().read_location("safeResolving/oas30SafeUrlResolvingWithPetstore.yaml", None, Some(&o));
    if let Some(m) = &result.messages {
        for message in m {
            assert!(
                message.contains("Server returned HTTP response code: 403")
                    || message.contains("URL is part of the explicit denylist. URL [https://petstore3.swagger.io/api/v3/openapi.json]"),
                "{message}"
            );
        }
    }
}

/// Test safe resolving with turned off safelyResolveURL option
#[test]
fn test31_safe_url_resolving_with_turned_off_safe_resolving() {
    let _g = common::setup(common::V3_MODULE);
    let _h = offline_remote();
    let o = options(|o| {
        o.resolve_fully = true;
        o.safely_resolve_url = false;
        o.remote_ref_allow_list = Some(vec![]);
        o.remote_ref_block_list = Some(vec!["petstore3.swagger.io".into()]);
    });
    let result = OpenAPIV3Parser::new().read_location("safeResolving/oas30SafeUrlResolvingWithPetstore.yaml", None, Some(&o));
    if let Some(m) = &result.messages {
        for message in m {
            assert!(message.contains("Server returned HTTP response code: 403"), "{message}");
        }
    }
}

/// Test safe resolving with localhost and blocked url
#[test]
fn test31_safe_url_resolving_with_localhost_and_blocked_url() {
    let _g = common::setup(common::V3_MODULE);
    let _h = offline_remote();
    let o = options(|o| {
        o.resolve_fully = true;
        o.safely_resolve_url = true;
    });
    let result = OpenAPIV3Parser::new().read_location("safeResolving/oas30SafeUrlResolvingWithLocalhost.yaml", None, Some(&o));
    if let Some(m) = &result.messages {
        for message in m {
            assert!(message.contains("Server returned HTTP response code: 403") || message.contains("IP is restricted"), "{message}");
        }
    }
}

/// Test safe resolving with localhost url
#[test]
fn test31_safe_url_resolving_with_localhost() {
    let _g = common::setup(common::V3_MODULE);
    let _h = offline_remote();
    let o = options(|o| {
        o.resolve_fully = true;
        o.safely_resolve_url = true;
        o.remote_ref_block_list = Some(vec!["petstore.swagger.io".into()]);
    });
    let error = "URL is part of the explicit denylist. URL [https://petstore.swagger.io/v2/swagger.json]";
    let result = OpenAPIV3Parser::new().read_location("safeResolving/oas30SafeUrlResolvingWithLocalhost.yaml", None, Some(&o));
    if let Some(m) = &result.messages {
        for message in m {
            assert!(
                message.contains("Server returned HTTP response code: 403") || message.contains("IP is restricted") || message.contains(error),
                "{message}"
            );
        }
    }
}

#[test]
fn test_issue1886() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| {
        o.resolve = true;
        o.flatten = true;
    });
    let parse_result = OpenAPIV3Parser::new().read_location("issue-1886/openapi.yaml", None, Some(&o));
    let open_api = parse_result.openapi.unwrap();
    let s = schemas(&open_api);
    assert!(s.get("Enum1").is_some());
    assert!(s.get("Enum1_1").is_none());
    let mut keys = s.keys();
    keys.sort();
    let mut expected: Vec<String> = [
        "ArrayPojo", "Enum1", "Enum2", "Enum3", "MapPojo", "SetPojo", "SimplePojo", "TransactionsPatchRequestBody", "additional-properties",
        "array-pojo", "locale-translation-item", "map-pojo", "set-pojo", "simple-pojo", "translation-item",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    expected.sort();
    assert_eq!(keys, expected);
}

#[test]
fn test_issue2081() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| o.resolve_fully = true);
    let parse_result = OpenAPIV3Parser::new().read_location("issue2081/openapi.yaml", None, Some(&o));
    let open_api = parse_result.openapi.unwrap();
    let _ = yaml_pretty(&open_api);
    let pet_create = schema(&open_api, "PetCreate");
    assert_eq!(pet_create.get_required().unwrap().len(), 1);
    assert_eq!(pet_create.get_properties().unwrap().len(), 2);
}

fn response_ref(o: &OpenAPI, path: &str, method: &str, code: &str) -> Option<String> {
    let item = o.get_paths().unwrap().get(path).unwrap();
    let op = match method {
        "get" => item.get_get(),
        "post" => item.get_post(),
        "put" => item.get_put(),
        "delete" => item.get_delete(),
        _ => unreachable!(),
    };
    op.unwrap().get_responses().unwrap().get(code).unwrap().get_ref()
}

const RESPONSE_REFS: &[(&str, &str, &str, &str)] = &[
    ("/users", "get", "400", "#/components/responses/BadRequest"),
    ("/users", "post", "400", "#/components/responses/BadRequest"),
    ("/users", "post", "422", "#/components/responses/UnprocessableEntity"),
    ("/users/{userId}", "get", "404", "#/components/responses/NotFound"),
    ("/users/{userId}", "put", "400", "#/components/responses/BadRequest"),
    ("/users/{userId}", "put", "404", "#/components/responses/NotFound"),
    ("/users/{userId}", "delete", "400", "#/components/responses/BadRequest"),
    ("/users/{userId}", "delete", "404", "#/components/responses/NotFound"),
];

/// responses should be inline
#[test]
fn test_fully_resolve_responses() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| {
        o.resolve = true;
        o.resolve_responses = true;
    });
    let open_api = OpenAPIV3Parser::new().read_location("resolve-responses-test.yaml", None, Some(&o)).openapi.unwrap();
    for (path, method, code, _) in RESPONSE_REFS {
        assert!(response_ref(&open_api, path, method, code).is_none(), "{path} {method} {code}");
    }
}

/// responses should not be inline
#[test]
fn test_resolve_responses_ref() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| o.resolve = true);
    let open_api = OpenAPIV3Parser::new().read_location("resolve-responses-test.yaml", None, Some(&o)).openapi.unwrap();
    for (path, method, code, expected) in RESPONSE_REFS {
        assert_eq!(response_ref(&open_api, path, method, code).as_deref(), Some(*expected), "{path} {method} {code}");
    }
}

#[test]
fn test_resolve_oas_with_flatten() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| {
        o.resolve = true;
        o.flatten = true;
    });
    let open_api = OpenAPIV3Parser::new().read_location("resolve-flatten-SH-configuration-test.yaml", None, Some(&o)).openapi.unwrap();
    let s = schemas(&open_api);
    assert!(s.get("#/components/schemas/inline_response_404").is_none());
    assert!(s.get("#/components/schemas/inline_response_200").is_none());
}

/// responses should be inline with using resolveFully = true
#[test]
fn test_resolve_fully_responses() {
    let _g = common::setup(common::V3_MODULE);
    let o = options(|o| {
        o.resolve = true;
        o.resolve_fully = true;
    });
    let open_api = OpenAPIV3Parser::new().read_location("resolve-responses-test.yaml", None, Some(&o)).openapi.unwrap();
    assert!(response_ref(&open_api, "/users", "get", "400").is_none());
}

struct StyleExplode {
    params: Vec<Parameter>,
    headers: JMap<swagger_parser::models::Header>,
    encoding: JMap<swagger_parser::models::Encoding>,
}

fn style_explode(explicit: bool) -> StyleExplode {
    let o = options(|o| o.explicit_style_and_explode = explicit);
    let open_api = OpenAPIV3Parser::new().read_location("style-explode.yaml", None, Some(&o)).openapi.unwrap();
    let item = open_api.get_paths().unwrap().get("/test").unwrap();
    let get = item.get_get().unwrap();
    StyleExplode {
        params: get.get_parameters().unwrap().to_vec(),
        headers: get.get_responses().unwrap().get("200").unwrap().get_headers().unwrap(),
        encoding: item.get_post().unwrap().get_request_body().unwrap().get_content().unwrap().get("multipart/form-data").unwrap().get_encoding().unwrap(),
    }
}

/// style and explode should not be set with explicitStyleAndExplode = false
#[test]
fn test_style_and_explode_not_explicit() {
    let _g = common::setup(common::V3_MODULE);
    let s = style_explode(false);
    let p = &s.params;
    assert_eq!(p[0].get_style().unwrap().to_string(), "form");
    assert_eq!(p[0].get_explode(), Some(true));
    assert_eq!(p[1].get_style().unwrap().to_string(), "spaceDelimited");
    assert!(p[1].get_explode().is_none());
    assert_eq!(p[2].get_explode(), Some(true));
    assert!(p[2].get_style().is_none());
    assert!(p[6].get_style().is_none());
    assert!(p[6].get_explode().is_none());
    let bar = s.headers.get("bar").unwrap();
    assert!(bar.get_explode().is_none());
    assert!(bar.get_style().is_none());
    let foo = s.headers.get("foo").unwrap();
    assert_eq!(foo.get_explode(), Some(false));
    assert!(foo.get_style().is_none());
    let enc = |k: &str| s.encoding.get(k).unwrap();
    assert_eq!(enc("file").get_style(), Some(EncodingStyle::Form));
    assert_eq!(enc("file").get_style().unwrap().to_string(), "form");
    assert_eq!(enc("file").get_explode(), Some(true));
    assert_eq!(enc("fileWithOnlyStyle").get_style().unwrap().to_string(), "form");
    assert!(enc("fileWithOnlyStyle").get_explode().is_none());
    assert_eq!(enc("fileWithOnlyExplode").get_explode(), Some(true));
    assert!(enc("fileWithOnlyExplode").get_style().is_none());
    assert!(enc("fileWithout").get_style().is_none());
    assert!(enc("fileWithout").get_explode().is_none());
}

/// style and explode should be set with explicitStyleAndExplode = true
#[test]
fn test_style_and_explode_explicit() {
    let _g = common::setup(common::V3_MODULE);
    let s = style_explode(true);
    let p = &s.params;
    assert_eq!(p[0].get_style().unwrap().to_string(), "form");
    assert_eq!(p[0].get_explode(), Some(true));
    assert_eq!(p[1].get_style().unwrap().to_string(), "spaceDelimited");
    assert_eq!(p[1].get_explode(), Some(false));
    assert_eq!(p[2].get_style().unwrap().to_string(), "form");
    assert_eq!(p[2].get_explode(), Some(true));
    assert_eq!(p[6].get_style().unwrap().to_string(), "simple");
    assert_eq!(p[6].get_explode(), Some(false));
    let bar = s.headers.get("bar").unwrap();
    assert_eq!(bar.get_explode(), Some(false));
    assert_eq!(bar.get_style().unwrap().to_string(), "simple");
    let foo = s.headers.get("foo").unwrap();
    assert_eq!(foo.get_explode(), Some(false));
    assert!(foo.get_style().is_none());
    let enc = |k: &str| s.encoding.get(k).unwrap();
    assert_eq!(enc("file").get_style().unwrap().to_string(), "form");
    assert_eq!(enc("file").get_explode(), Some(true));
    assert_eq!(enc("fileWithOnlyStyle").get_style().unwrap().to_string(), "form");
    assert!(enc("fileWithOnlyStyle").get_explode().is_none());
    assert_eq!(enc("fileWithOnlyExplode").get_explode(), Some(true));
    assert_eq!(enc("fileWithOnlyExplode").get_style().unwrap().to_string(), "form");
    assert_eq!(enc("fileWithout").get_style().unwrap().to_string(), "form");
    assert!(enc("fileWithout").get_explode().is_none());
}

/// null version should cause a message
#[test]
fn test_version() {
    let _g = common::setup(common::V3_MODULE);
    let o = ParseOptions::default();
    let parse_result = OpenAPIV3Parser::new().read_location("version-missing.yaml", None, Some(&o));
    assert_eq!(messages(&parse_result)[0], "attribute info.version is missing");
}
