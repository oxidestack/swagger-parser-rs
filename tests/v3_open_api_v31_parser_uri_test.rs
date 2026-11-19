//! Port of `io.swagger.v3.parser.test.OpenAPIV31ParserUriTest`: parsing class path and file URIs must not
//! produce "file not found" messages.

mod common;

use swagger_parser::core::{ParseOptions, SwaggerParseResult};
use swagger_parser::java::classpath::find_resource;
use swagger_parser::parser::OpenAPIV3Parser;

/// `getClass().getResource("/3.1.0/basic.yaml")`: the class path file.
fn resource(name: &str) -> swagger_parser::java::path::JavaPath {
    find_resource(name.trim_start_matches('/')).expect("resource")
}

#[test]
fn resolve_file_input() {
    let _g = common::setup(common::V3_MODULE);
    // URL.toURI() of a file resource: "file:/abs/path"
    let uri_string = resource("/3.1.0/basic.yaml").to_uri().replacen("file://", "file:", 1);
    assert!(uri_string.starts_with("file:"));
    let options = ParseOptions { resolve: true, ..Default::default() };
    let result = OpenAPIV3Parser::new().read_location(&uri_string, None, Some(&options));
    validate_parse_result(&result, "(No such file or directory)");
}

#[test]
fn resolve_classpath_input() {
    let _g = common::setup(common::V3_MODULE);
    let content = resource("/3.1.0/basic.yaml").read_to_string().unwrap();
    let options = ParseOptions { resolve: true, ..Default::default() };
    let result = OpenAPIV3Parser::new().read_contents_at(&content, None, Some(&options), Some("classpath:/3.1.0/basic.yaml"));
    validate_parse_result(&result, "Could not find classpath:/3.1.0/basic.yaml");
}

fn validate_parse_result(result: &SwaggerParseResult, check_for_message: &str) {
    let no_such_file_message = result.messages.as_ref().unwrap().iter().find(|m| m.contains(check_for_message));
    assert!(no_such_file_message.is_none(), "{no_such_file_message:?}");
}
