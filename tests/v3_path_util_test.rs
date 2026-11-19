//! Port of `io.swagger.v3.parser.util.PathUtilTest`.

mod common;

use swagger_parser::java::path::JavaPath;
use swagger_parser::parser::path_utils::get_parent_directory_of_file;

#[test]
fn test_get_parent_directory_of_file() {
    let _g = common::setup(common::V3_MODULE);
    let actual_result = get_parent_directory_of_file("src/test/resources/test.yaml").unwrap().unwrap().to_string();
    let expected_result = JavaPath::new("src/test/resources").to_absolute_path().to_string();
    assert_eq!(actual_result, expected_result);
}

#[test]
fn test_get_parent_directory_of_non_existent_file() {
    let _g = common::setup(common::V3_MODULE);
    let result = get_parent_directory_of_file("src/test/resources/parent.json").unwrap();
    assert!(result.is_none());
}
