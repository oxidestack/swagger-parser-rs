//! Port of `io.swagger.v3.parser.util.RefUtilsTest` (`@Mocked RemoteUrl`, `IOUtils`, `Files`, `ClasspathHelper`;
//! `@Injectable Path`s). The file-reading tests mock `parentDirectory.resolve(filePath).normalize()`,
//! `Files.exists` and `IOUtils.toString`; here a real parent directory provides the same answers: a file whose
//! contents are the recorded result, or (for the `IOException`) a directory in its place.

mod common;

use swagger_parser::core::AuthorizationValue;
use swagger_parser::java::path::JavaPath;
use swagger_parser::java::JavaException;
use swagger_parser::parser::hooks::{self, Hooks};
use swagger_parser::parser::processors::external_ref::join;
use swagger_parser::parser::ref_utils::*;
use swagger_parser_safe_url_resolver::PermittedUrlsChecker;

/// `auths` (the constructor fills a local list, so the field stays null)
const AUTHS: &[AuthorizationValue] = &[];

fn checker() -> PermittedUrlsChecker {
    PermittedUrlsChecker::default()
}

fn do_compute_definition_name_test_case(r: &str, expected: &str) {
    assert_eq!(expected, compute_definition_name(r).unwrap());
}

#[test]
fn test_compute_definition_name() {
    let _g = common::setup(common::V3_MODULE);
    // URL refs
    do_compute_definition_name_test_case("http://my.company.com/path/to/file.json", "file");
    do_compute_definition_name_test_case("http://my.company.com/path/to/file.json#/foo", "foo");
    do_compute_definition_name_test_case("http://my.company.com/path/to/file.json#/foo/bar", "bar");
    do_compute_definition_name_test_case("http://my.company.com/path/to/file.json#/foo/bar/hello", "hello");

    do_compute_definition_name_test_case("http://my.company.com/path/to/file", "file");
    do_compute_definition_name_test_case("http://my.company.com/path/to/file#/foo", "foo");
    do_compute_definition_name_test_case("http://my.company.com/path/to/file#/foo/bar", "bar");
    do_compute_definition_name_test_case("http://my.company.com/path/to/file#/foo/bar/hello", "hello");

    do_compute_definition_name_test_case("http://my.company.com/path/to/file.yaml", "file");
    do_compute_definition_name_test_case("http://my.company.com/path/to/file.yaml#/foo", "foo");
    do_compute_definition_name_test_case("http://my.company.com/path/to/file.yaml#/foo/bar", "bar");
    do_compute_definition_name_test_case("http://my.company.com/path/to/file.yaml#/foo/bar/hello", "hello");

    // Relative file refs
    do_compute_definition_name_test_case("./path/to/file.json", "file");
    do_compute_definition_name_test_case("./path/to/file.json#/foo", "foo");
    do_compute_definition_name_test_case("./path/to/file.json#/foo/bar", "bar");
    do_compute_definition_name_test_case("./path/to/file.json#/foo/bar/hello", "hello");

    do_compute_definition_name_test_case("./path/to/file.yaml", "file");
    do_compute_definition_name_test_case("./path/to/file.yaml#/foo", "foo");
    do_compute_definition_name_test_case("./path/to/file.yaml#/foo/bar", "bar");
    do_compute_definition_name_test_case("./path/to/file.yaml#/foo/bar/hello", "hello");

    do_compute_definition_name_test_case("./path/to/file", "file");
    do_compute_definition_name_test_case("./path/to/file#/foo", "foo");
    do_compute_definition_name_test_case("./path/to/file#/foo/bar", "bar");
    do_compute_definition_name_test_case("./path/to/file#/foo/bar/hello", "hello");
}

#[test]
fn test_is_an_external_ref_format() {
    let _g = common::setup(common::V3_MODULE);
    for value in [RefFormat::Url, RefFormat::Relative, RefFormat::Internal] {
        match value {
            RefFormat::Url | RefFormat::Relative => assert!(is_an_external_ref_format(value)),
            RefFormat::Internal => assert!(!is_an_external_ref_format(value)),
        }
    }
}

#[test]
fn test_read_external_ref_url_format() {
    let _g = common::setup(common::V3_MODULE);
    let url = "http://my.company.com/path/to/file.json";
    let expected_result = "really good json";
    let remote = common::mock_remote_url(&[(url, expected_result)]);

    let actual = read_external_ref(url, RefFormat::Url, AUTHS, None, Some(&checker())).unwrap();
    assert_eq!(actual.as_deref(), Some(expected_result));
    assert_eq!(remote.count(url), 1);
}

#[test]
fn test_read_external_ref_url_format_exception_thrown() {
    let _g = common::setup(common::V3_MODULE);
    let url = "http://my.company.com/path/to/file.json";
    let calls = std::rc::Rc::new(std::cell::RefCell::new(0));
    let c = calls.clone();
    // RemoteUrl.urlToString(url, auths, checker); result = new Exception()
    let _h = hooks::install(Hooks {
        url_to_string: Some(std::rc::Rc::new(move |u: &str, _a: &[AuthorizationValue]| {
            assert_eq!(u, "http://my.company.com/path/to/file.json");
            *c.borrow_mut() += 1;
            Some(Err(JavaException { class: "java.lang.Exception".into(), message: None, cause: None }))
        })),
        ..Default::default()
    });
    let e = read_external_ref(url, RefFormat::Url, AUTHS, None, Some(&checker())).unwrap_err();
    // thrown.expect(RuntimeException.class)
    assert_eq!(e.class, "java.lang.RuntimeException");
    assert_eq!(*calls.borrow(), 1);
}

/// A fresh parent directory (the `@Injectable Path parentDirectory`)
fn temp_dir(name: &str) -> String {
    let d = std::env::temp_dir().join(format!("ref-utils-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d.to_string_lossy().into_owned()
}

#[test]
fn test_read_external_ref_relative_file_format() {
    let _g = common::setup(common::V3_MODULE);
    let file_path = "file.json";
    let expected_result = "really good json";
    let dir = temp_dir("relative");
    // parentDirectory.resolve(filePath).normalize() → pathToUse; Files.exists(pathToUse) → true;
    // IOUtils.toString(..) → expectedResult
    std::fs::write(format!("{dir}/{file_path}"), expected_result).unwrap();
    let parent_directory = JavaPath::new(&dir);

    let actual = read_external_ref(file_path, RefFormat::Relative, AUTHS, Some(&parent_directory), Some(&checker())).unwrap();
    assert_eq!(actual.as_deref(), Some(expected_result));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn test_read_external_ref_relative_file_format_exception_thrown() {
    let _g = common::setup(common::V3_MODULE);
    let file_path = "file.json";
    let dir = temp_dir("relative-exception");
    // Files.exists(pathToUse) → true; IOUtils.toString(..) throws an IOException: reading a directory does
    std::fs::create_dir(format!("{dir}/{file_path}")).unwrap();
    let parent_directory = JavaPath::new(&dir);

    let e = read_external_ref(file_path, RefFormat::Relative, AUTHS, Some(&parent_directory), Some(&checker())).unwrap_err();
    // ((Exception) o).getCause().getClass().equals(IOException.class)
    assert_eq!(e.cause.as_ref().map(|c| c.class.as_str()), Some("java.io.IOException"));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn test_read_external_ref_internal_format() {
    let _g = common::setup(common::V3_MODULE);
    let file = "#/defintiions/foo";
    match read_external_ref(file, RefFormat::Internal, AUTHS, None, Some(&checker())) {
        Ok(_) => panic!("Should have thrown an exception"),
        Err(e) => assert_eq!(e.class, "java.lang.RuntimeException"),
    }
}

fn j(source: &str, fragment: &str) -> String {
    join(Some(source), Some(fragment)).unwrap()
}

#[test]
fn test_path_join1() {
    let _g = common::setup(common::V3_MODULE);
    // simple
    assert_eq!(j("http://foo.bar.com", "fun"), "http://foo.bar.com/fun");
    assert_eq!(j("http://foo.bar.com/", "fun"), "http://foo.bar.com/fun");
    assert_eq!(j("http://foo.bar.com/", "/fun"), "http://foo.bar.com/fun");
    assert_eq!(j("http://foo.bar.com", "/fun"), "http://foo.bar.com/fun");

    // relative to host
    assert_eq!(j("http://foo.bar.com/foo/bar", "/fun"), "http://foo.bar.com/fun");
    assert_eq!(j("http://foo.bar.com/foo/bar#/baz/bat", "/fun"), "http://foo.bar.com/fun");
    assert_eq!(j("http://foo.bar.com/foo/bar#/baz/bat", "/fun#for/all"), "http://foo.bar.com/fun#for/all");

    // relative
    assert_eq!(j("http://foo.bar.com", "./fun"), "http://foo.bar.com/fun");
    assert_eq!(j("http://foo.bar.com/veryFun", "./fun"), "http://foo.bar.com/fun");
    assert_eq!(j("http://foo.bar.com/veryFun/", "../fun#nothing"), "http://foo.bar.com/fun#nothing");
    assert_eq!(j("http://foo.bar.com/veryFun/notFun", "../fun#/it/is/fun"), "http://foo.bar.com/fun#/it/is/fun");

    // with file extensions
    assert_eq!(j("http://foo.bar.com/baz/bat.yaml", "../fun/times.yaml"), "http://foo.bar.com/fun/times.yaml");

    // hashes
    assert_eq!(j("http://foo.bar.com/veryFun/", "../fun#/it/is/fun"), "http://foo.bar.com/fun#/it/is/fun");

    // relative locations
    assert_eq!(j("./foo#/definitions/Foo", "./bar#/definitions/Bar"), "./bar#/definitions/Bar");
}

fn b(root: &str, relative: &str) -> String {
    build_url(Some(root), Some(relative)).unwrap().unwrap()
}

#[test]
fn test_path_join2() {
    let _g = common::setup(common::V3_MODULE);
    assert_eq!(b("http://foo.bar.com/my/dir/file.yaml", "../newFile.yaml"), "http://foo.bar.com/my/newFile.yaml");
    assert_eq!(b("http://foo.bar.com/my/dir/file.yaml", "../../newFile.yaml"), "http://foo.bar.com/newFile.yaml");
    assert_eq!(b("http://foo.bar.com/my/dir/file.yaml", "./newFile.yaml"), "http://foo.bar.com/my/dir/newFile.yaml");
    assert_eq!(b("http://foo.bar.com/my/dir/file.yaml", "../second/newFile.yaml"), "http://foo.bar.com/my/second/newFile.yaml");
    assert_eq!(b("http://foo.bar.com/my/dir/file.yaml", "../../otherDir/newFile.yaml"), "http://foo.bar.com/otherDir/newFile.yaml");
    assert_eq!(b("http://foo.bar.com/file.yaml", "./newFile.yaml"), "http://foo.bar.com/newFile.yaml");
    assert_eq!(b("http://foo.bar.com/my/dir/file.yaml", "/newFile.yaml"), "http://foo.bar.com/newFile.yaml");
    assert_eq!(b("http://foo.bar.com/my/dir/file.yaml", "/my/newFile.yaml"), "http://foo.bar.com/my/newFile.yaml");
}

#[test]
fn test_path_join_issue1745() {
    let _g = common::setup(common::V3_MODULE);
    assert_eq!(b("http://foo.bar.com/my/dir/file.yaml", "./second/../newFile.yaml"), "http://foo.bar.com/my/dir/newFile.yaml");
    assert_eq!(b("http://foo.bar.com/my/dir/", "./second/../newFile.yaml"), "http://foo.bar.com/my/dir/newFile.yaml");
    assert_eq!(b("http://foo.bar.com/my/dir/file.yaml", "/second/../newFile.yaml"), "http://foo.bar.com/second/../newFile.yaml");
}

#[test]
fn should_return_empty_external_path_for_internal_reference() {
    let _g = common::setup(common::V3_MODULE);
    assert!(get_external_path(Some("#/components/test")).is_none());
}

#[test]
fn should_return_empty_external_path_for_null_reference() {
    let _g = common::setup(common::V3_MODULE);
    assert!(get_external_path(None).is_none());
}

#[test]
fn should_return_empty_external_path_for_empty_reference() {
    let _g = common::setup(common::V3_MODULE);
    assert!(get_external_path(Some("")).is_none());
}

#[test]
fn should_return_empty_external_path_for_invalid_reference() {
    let _g = common::setup(common::V3_MODULE);
    assert!(get_external_path(Some("test")).is_none());
}

#[test]
fn should_return_external_path_for_file_reference() {
    let _g = common::setup(common::V3_MODULE);
    let external_path = get_external_path(Some("test.yaml#/components/test"));
    assert_eq!(external_path.as_deref(), Some("test.yaml"));
}

#[test]
fn should_return_external_path_for_http_reference() {
    let _g = common::setup(common::V3_MODULE);
    let external_path = get_external_path(Some("http://localhost/schema.json#/components/test"));
    assert_eq!(external_path.as_deref(), Some("http://localhost/schema.json"));
}
