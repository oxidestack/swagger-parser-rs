//! Port of `io.swagger.v3.parser.util.ClasspathHelperTest`.

mod common;

use swagger_parser::parser::ref_utils::load_file_from_classpath;

#[test]
fn test_load_file_from_classpath() {
    let _g = common::setup(common::V3_MODULE);
    let contents = load_file_from_classpath("classpathTest.txt").unwrap();
    assert_eq!(contents, "How now brown cow?");
}

#[test]
fn test_load_file_from_classpath_doesnt_exist() {
    let _g = common::setup(common::V3_MODULE);
    // expectedExceptions = RuntimeException.class
    let e = load_file_from_classpath("nothing.txt").unwrap_err();
    assert_eq!(e.class, "java.lang.RuntimeException");
}

#[test]
fn test_load_file_from_classpath_handles_windows_paths_ok() {
    let _g = common::setup(common::V3_MODULE);
    let contents = load_file_from_classpath("\\issue-1878\\test.txt").unwrap();
    assert_eq!(contents, "Test content");
}
