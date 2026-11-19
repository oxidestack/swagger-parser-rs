//! Port of `io.swagger.v3.parser.test.OpenAPIV31ParserFSFullTest`.

mod common;

use swagger_parser::core::ParseOptions;
use swagger_parser::models::yaml31_pretty;
use swagger_parser::parser::OpenAPIV3Parser;

#[test]
fn test_full() {
    let _g = common::setup(common::V3_MODULE);
    let p = ParseOptions { resolve: true, ..Default::default() };
    let result = OpenAPIV3Parser::new().read_location("3.1.0/dereference/fullFS/root.json", None, Some(&p));
    assert_eq!(yaml31_pretty(&result.openapi.unwrap()).unwrap(), common::read_file("src/test/resources/3.1.0/dereference/fullFS/dereferenced.yaml"));
}

#[test]
fn test_custom_url_resolver() {
    let _g = common::setup(common::V3_MODULE);
    let p = ParseOptions { resolve: true, ..Default::default() };
    let result = OpenAPIV3Parser::new().read_location("3.1.0/dereference/custom/root.json", None, Some(&p));
    assert_eq!(yaml31_pretty(&result.openapi.unwrap()).unwrap(), common::read_file("src/test/resources/3.1.0/dereference/custom/dereferenced.yaml"));
}
