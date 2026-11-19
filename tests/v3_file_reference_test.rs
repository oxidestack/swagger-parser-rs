//! Port of `io.swagger.v3.parser.test.FileReferenceTest`.

mod common;

use swagger_parser::core::{ParseOptions, SwaggerParseResult};
use swagger_parser::models::*;
use swagger_parser::parser::OpenAPIV3Parser;

fn read(location: &str) -> SwaggerParseResult {
    let mut options = ParseOptions::new();
    options.resolve = true;
    OpenAPIV3Parser::new().read_location(location, None, Some(&options))
}

fn schemas(o: &OpenAPI) -> swagger_parser::java::JMap<Schema> {
    o.get_components().unwrap().get_schemas().unwrap()
}

fn props_len(o: &OpenAPI, name: &str) -> usize {
    schemas(o).get(name).unwrap().get_properties().unwrap().len()
}

#[test]
fn test_issue306() {
    let _g = common::setup(common::V3_MODULE);
    let result = read("./src/test/resources/nested-file-references/issue-306.yaml");
    let swagger = result.openapi.expect("openAPI");
    assert!(schemas(&swagger).len() == 5);
    // resolved from `$ref: './book.yaml'`
    assert!(schemas(&swagger).get("Inventory").is_some());
    // resolved from `$ref: 'book.yaml'`
    assert!(schemas(&swagger).get("Orders").is_some());
    // copied from `./book.yaml`
    assert!(schemas(&swagger).get("book").is_some());
}

#[test]
fn test_issue308() {
    let _g = common::setup(common::V3_MODULE);
    let swagger = read("./src/test/resources/nested-file-references/issue-308.yaml").openapi.expect("openAPI");
    assert!(schemas(&swagger).len() == 2);
    assert!(props_len(&swagger, "Paging") == 1);
}

#[test]
fn test_issue310() {
    let _g = common::setup(common::V3_MODULE);
    let swagger = read("./src/test/resources/nested-file-references/issue-310.yaml").openapi.expect("openAPI");
    assert!(schemas(&swagger).len() == 2);
    assert!(props_len(&swagger, "Paging") == 1);
}

#[test]
fn test_issue312() {
    let _g = common::setup(common::V3_MODULE);
    let swagger = read("./src/test/resources/nested-file-references/issue-312.yaml").openapi.expect("openAPI");
    let path = swagger.get_paths().unwrap().get("/events").expect("/events");
    let get = path.get_get().expect("get");
    assert_eq!(get.get_operation_id().as_deref(), Some("getEvents"));
    // Yaml.prettyPrint(swagger)
    println!("{}", yaml_pretty(&swagger).unwrap_or_else(|| "null".into()));
    assert!(schemas(&swagger).len() == 2);
    assert!(props_len(&swagger, "Paging") == 1);
}

#[test]
fn test_issue314() {
    let _g = common::setup(common::V3_MODULE);
    let swagger = read("./src/test/resources/nested-file-references/issue-314.yaml").openapi.expect("openAPI");
    let path = swagger.get_paths().unwrap().get("/events").expect("/events");
    let get = path.get_get().expect("get");
    assert_eq!(get.get_operation_id().as_deref(), Some("getEvents"));
    assert!(schemas(&swagger).len() == 3);
    assert!(props_len(&swagger, "Foobar") == 1);
    assert!(props_len(&swagger, "StatusResponse") == 1);
    assert!(props_len(&swagger, "Paging") == 1);
}

#[test]
fn test_issue316() {
    let _g = common::setup(common::V3_MODULE);
    let swagger = read("./src/test/resources/nested-file-references/issue-316.yaml").openapi.expect("openAPI");
    let path = swagger.get_paths().unwrap().get("/events").expect("/events");
    let get = path.get_get().expect("get");
    assert_eq!(get.get_operation_id().as_deref(), Some("getEvents"));
    assert!(schemas(&swagger).len() == 3);
    assert!(props_len(&swagger, "Foobar") == 1);
    assert!(props_len(&swagger, "StatusResponse") == 1);
    assert!(props_len(&swagger, "Paging2") == 2);
    let model = schemas(&swagger).get("Paging2").unwrap();
    let property = model.get_properties().unwrap().get("foobar").unwrap();
    assert!(property.get_ref().is_some());
    assert_eq!(property.get_ref().as_deref(), Some("#/components/schemas/Foobar"));
}

#[test]
fn test_issue323() {
    let _g = common::setup(common::V3_MODULE);
    let swagger = read("./src/test/resources/nested-file-references/issue-323.yaml").openapi.expect("openAPI");
    assert!(swagger.get_paths().unwrap().get("/events").is_some());
    assert!(schemas(&swagger).get("StatusResponse").is_some());
    assert!(schemas(&swagger).get("Paging").is_some());
    assert!(schemas(&swagger).get("Foobar").is_some());
}

#[test]
fn test_issue289() {
    let _g = common::setup(common::V3_MODULE);
    let swagger = read("./src/test/resources/issue-289.yaml").openapi.expect("openAPI");
    assert!(swagger.get_paths().unwrap().get("/foo").unwrap().get_get().is_some());
}

#[test]
fn test_issue822() {
    let _g = common::setup(common::V3_MODULE);
    let swagger = read("./src/test/resources/issue-822.yaml").openapi.unwrap();
    let paths = swagger.get_paths().unwrap();
    assert!(paths.get("/foo").unwrap().get_get().is_some());
    assert!(paths.get("/bar/wtf").unwrap().get_get().is_some());
    assert!(paths.get("/bar/haha").unwrap().get_get().is_none());
    assert!(paths.get("/wtf/{bar}").unwrap().get_get().is_some());
    assert!(paths.get("/haha/{bar}").unwrap().get_get().is_some());
    assert!(paths.get("/haha2/{bar}").unwrap().get_get().is_none());
}

#[test]
fn test_issue336() {
    let _g = common::setup(common::V3_MODULE);
    let swagger = read("./src/test/resources/nested-file-references/issue-336.json").openapi.expect("openAPI");
    assert!(swagger.get_paths().is_some());
}

#[test]
fn test_issue340() {
    let _g = common::setup(common::V3_MODULE);
    let swagger = read("./src/test/resources/nested-file-references/issue-340.json").openapi.expect("openAPI");
    assert!(!schemas(&swagger).get("BarData").unwrap().get_ref().is_some());
}

#[test]
fn test_issue304() {
    let _g = common::setup(common::V3_MODULE);
    let result = read("./src/test/resources/nested-file-references/issue-304.json");
    assert!(result.openapi.unwrap().get_components().unwrap().get_schemas().is_some());
}

#[test]
fn test_all_of_flat_and_nested() {
    let _g = common::setup(common::V3_MODULE);
    for path in ["./src/test/resources/allOf-properties-ext-ref/models/swagger.json", "./src/test/resources/allOf-properties-ext-ref/swagger.json"] {
        let swagger = OpenAPIV3Parser::new().read(path).unwrap();
        assert_eq!(3, schemas(&swagger).len());
        let composed_model = schemas(&swagger).get("record").unwrap();
        // (ComposedSchema) cast
        assert!(composed_model.is_composed_schema());
        let all_of = composed_model.get_all_of().unwrap();
        assert_eq!(all_of.get(0).unwrap().get_ref().as_deref(), Some("#/components/schemas/pet"));
        let props = all_of.get(1).unwrap().get_properties().unwrap();
        assert_eq!(props.get("mother").unwrap().get_ref().as_deref(), Some("#/components/schemas/pet"));
        let siblings = props.get("siblings").unwrap();
        // (ArraySchema) cast
        assert!(siblings.is_array_schema());
        assert_eq!(siblings.get_items().unwrap().get_ref().as_deref(), Some("#/components/schemas/pet"));
    }
}

#[test]
fn test_issue421() {
    let _g = common::setup(common::V3_MODULE);
    let swagger = read("./src/test/resources/nested-file-references/issue-421.yaml").openapi.expect("openAPI");
    let paths = swagger.get_paths().unwrap();
    let pet = paths.get("/pet/{petId}").expect("/pet/{petId}");
    let get = pet.get_get().expect("get");
    let params = get.get_parameters().expect("parameters");
    assert!(params.len() == 1);
    assert!(params.get(0).unwrap().get_name().as_deref() == Some("petId"));
    assert!(schemas(&swagger).get("Pet").is_some());
    assert!(props_len(&swagger, "Pet") == 6);

    let post = pet.get_post().expect("post");
    let post_params = post.get_parameters().expect("parameters");
    assert!(post_params.len() == 1);
    let body = post.get_request_body();
    assert!(body.is_some());
    assert!(body.as_ref().unwrap().get_ref().is_some());
    assert_eq!(body.as_ref().unwrap().get_ref().as_deref(), Some("#/components/requestBodies/requestBody"));
    assert!(body.unwrap().get_ref().as_deref() == Some("#/components/requestBodies/requestBody"));

    let order = paths.get("/store/order").expect("/store/order");
    let order_post = order.get_post().expect("post");
    let order_body = order_post.get_request_body().expect("requestBody");
    let s = order_body.get_content().unwrap().get("application/json").unwrap().get_schema().expect("schema");
    assert!(s.get_ref().is_some());
    assert!(s.get_ref().as_deref() == Some("#/components/schemas/Order"));

    assert!(schemas(&swagger).get("Order").is_some());
    assert!(props_len(&swagger, "Order") == 6);
}

#[test]
fn test_relative_ref_issue421() {
    let _g = common::setup(common::V3_MODULE);
    let swagger = read("./src/test/resources/main.yaml").openapi.expect("openAPI");
    let pets = swagger.get_paths().unwrap().get("pets").expect("pets");
    let get = pets.get_get().expect("get");
    let responses = get.get_responses().expect("responses");
    let r200 = responses.get("200").expect("200");
    let s = r200.get_content().unwrap().get("*/*").unwrap().get_schema().expect("schema");
    assert!(s.get_ref().is_some());
    assert_eq!(s.get_ref().as_deref(), Some("#/components/schemas/Pet"));
    assert!(schemas(&swagger).get("Pet").is_some());
    assert!(props_len(&swagger, "Pet") == 2);
}
