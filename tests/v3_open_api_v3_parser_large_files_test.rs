//! Port of `io.swagger.v3.parser.OpenAPIV3ParserLargeFilesTest`.

mod common;

use swagger_parser::core::ParseOptions;
use swagger_parser::parser::deserialization_utils::update_options;
use swagger_parser::parser::OpenAPIV3Parser;

/// `@BeforeClass init` / `@AfterClass cleanUpAfterAllTests`. The `maxYamlCodePoints` system property is read
/// from the process environment by the port (`ReferenceVisitor.getYaml()`).
struct LargeYaml;

impl LargeYaml {
    fn init() -> LargeYaml {
        // SAFETY: the test runs alone (common::setup serializes the tests of this binary)
        unsafe { std::env::set_var("maxYamlCodePoints", "10000000") };
        update_options(|o| o.max_yaml_code_points = 10_000_000);
        LargeYaml
    }
}

impl Drop for LargeYaml {
    fn drop(&mut self) {
        // SAFETY: as above
        unsafe { std::env::remove_var("maxYamlCodePoints") };
        update_options(|o| o.max_yaml_code_points = 3 * 1024 * 1024);
    }
}

#[test]
fn issue2059() {
    let _g = common::setup(common::V3_MODULE);
    let _class = LargeYaml::init();
    let mut options = ParseOptions::new();
    options.resolve = true;
    let parse_result = OpenAPIV3Parser::new().read_location("issue2059/largeFile.yaml", None, Some(&options));
    assert!(parse_result.openapi.is_some());
    assert!(!parse_result.messages.clone().unwrap_or_default().iter().any(|m| m.contains("exceeds the limit: 3145728")));
}
