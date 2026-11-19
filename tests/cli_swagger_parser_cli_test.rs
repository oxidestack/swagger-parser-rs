//! Port of swagger-parser-cli's SwaggerParserCLITest.
//!
//! The Java tests write their outputs under the module's `target/test-classes`; here the module directory is
//! the read-only upstream checkout, so those paths live under `CARGO_TARGET_TMPDIR` instead, in a directory of
//! each test's own (named by the process and the test), so that tests writing the same file name, in this
//! process or in another run of this file at the same time, never see or remove each other's output.

mod common;

use std::path::PathBuf;
use swagger_parser::cli::{self, Namespace};

/// One test's `target/test-classes` of the Java module, removed when the test ends
struct TestClasses(PathBuf);

impl TestClasses {
    fn new(test: &str) -> Self {
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join("swagger-parser-cli")
            .join(format!("{}-{test}", std::process::id()));
        // a directory left by an earlier process with the same id is not this run's output
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("test-classes")).unwrap();
        TestClasses(root)
    }

    /// `target/test-classes/<name>`
    fn out(&self, name: &str) -> String {
        self.0.join("test-classes").join(name).to_string_lossy().into_owned()
    }
}

impl Drop for TestClasses {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn list(v: &Option<Vec<String>>) -> String {
    common::java_list(v.as_deref().unwrap())
}

#[test]
fn validate_ok_from_location_test() {
    let _g = common::setup(common::CLI_MODULE);
    let ns = Namespace::new(&[("i", "src/test/resources/fileWithNoErrorMessages.yaml")]);
    assert!(cli::read_from_location(&ns).unwrap().is_empty());
}

#[test]
fn validate_error_from_location_test() {
    let _g = common::setup(common::CLI_MODULE);
    let ns = Namespace::new(&[("i", "src/test/resources/fileWithValidationErrorMessages.yaml")]);
    assert_eq!(list(&cli::read_from_location(&ns)), "[attribute info.version is missing, attribute paths.'/cu'(post).responses.200.description is missing]");
}

#[test]
fn validate_error_test() {
    let _g = common::setup(common::CLI_MODULE);
    let dir = TestClasses::new("validate_error_test");
    let (o, l) = (dir.out("parsedSpec.yaml"), dir.out("errorLogs.yaml"));
    let ns = Namespace::new(&[
        ("i", "src/test/resources/fileWithValidationErrorMessages.yaml"),
        ("resolve", "true"),
        ("resolvefully", "true"),
        ("o", &o),
        ("l", &l),
    ]);
    assert_eq!(list(&cli::read_from_location(&ns)), "[attribute info.version is missing, attribute paths.'/cu'(post).responses.200.description is missing]");
}

#[test]
fn validate_file_not_found_in_location_test() {
    let _g = common::setup(common::CLI_MODULE);
    let dir = TestClasses::new("validate_file_not_found_in_location_test");
    let l = dir.out("errorLogs.yaml");
    let ns = Namespace::new(&[("i", "src/test/resources/WrongLocation.yaml"), ("l", &l)]);
    let messages = cli::read_from_location(&ns).unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(common::java_list(&messages), "[Unable to read location `src/test/resources/WrongLocation.yaml`]");
}

#[test]
fn validate_ok_from_location_with_resolve_option_test() {
    let _g = common::setup(common::CLI_MODULE);
    let ns = Namespace::new(&[("i", "src/test/resources/internal-references-in-external-files/main.yaml"), ("resolve", "true")]);
    assert!(cli::read_from_location(&ns).unwrap().is_empty());
}

#[test]
fn validate_ok_from_location_with_resolve_fully_option_test_yaml() {
    let _g = common::setup(common::CLI_MODULE);
    let dir = TestClasses::new("validate_ok_from_location_with_resolve_fully_option_test_yaml");
    let o = dir.out("parsedSpec.yaml");
    let ns = Namespace::new(&[
        ("i", "src/test/resources/internal-references-in-external-files/main.yaml"),
        ("resolve", "true"),
        ("resolvefully", "true"),
        ("yaml", "true"),
        ("o", &o),
    ]);
    assert!(cli::read_from_location(&ns).unwrap().is_empty());
    assert!(PathBuf::from(&o).exists());
}

#[test]
fn validate_ok_from_location_with_resolve_fully_option_test_json() {
    let _g = common::setup(common::CLI_MODULE);
    let dir = TestClasses::new("validate_ok_from_location_with_resolve_fully_option_test_json");
    let o = dir.out("parsedSpec.json");
    let ns = Namespace::new(&[
        ("i", "src/test/resources/internal-references-in-external-files/main.yaml"),
        ("resolve", "true"),
        ("resolvefully", "true"),
        ("json", "true"),
        ("o", &o),
    ]);
    assert!(cli::read_from_location(&ns).unwrap().is_empty());
    assert!(PathBuf::from(&o).exists());
}

#[test]
fn validate_ok_from_location_with_only_resolve_fully_option_test() {
    let _g = common::setup(common::CLI_MODULE);
    let ns = Namespace::new(&[("i", "src/test/resources/internal-references-in-external-files/main.yaml"), ("resolvefully", "true")]);
    assert!(cli::read_from_location(&ns).unwrap().is_empty());
}

#[test]
fn validate_ok_from_location_with_flatten_option_test() {
    let _g = common::setup(common::CLI_MODULE);
    let dir = TestClasses::new("validate_ok_from_location_with_flatten_option_test");
    let (o, l) = (dir.out("parsedSpec.yaml"), dir.out("errorLogs.yaml"));
    let ns = Namespace::new(&[
        ("i", "src/test/resources/internal-references-in-external-files/main.yaml"),
        ("resolve", "true"),
        ("resolvefully", "true"),
        ("flatten", "true"),
        ("o", &o),
        ("l", &l),
    ]);
    assert!(cli::read_from_location(&ns).unwrap().is_empty());
}

#[test]
fn validate_ok_from_location_with_resolve_fully_option_test_yaml_main() {
    let _g = common::setup(common::CLI_MODULE);
    let dir = TestClasses::new("validate_ok_from_location_with_resolve_fully_option_test_yaml_main");
    let o = dir.out("parsedSpec.yaml");
    let args: Vec<String> = vec![
        "-i=src/test/resources/internal-references-in-external-files/main.yaml".into(),
        "-resolve".into(),
        "-resolveFully".into(),
        "-yaml".into(),
        format!("-o={o}"),
    ];
    cli::main(&args);
    assert!(PathBuf::from(&o).exists());
}

#[test]
fn validate_ok_from_location_with_resolve_fully_option_test_json_main() {
    let _g = common::setup(common::CLI_MODULE);
    let dir = TestClasses::new("validate_ok_from_location_with_resolve_fully_option_test_json_main");
    let o = dir.out("parsedSpec.json");
    let args: Vec<String> = vec![
        "-i=src/test/resources/internal-references-in-external-files/main.yaml".into(),
        "-resolve".into(),
        "-resolveFully".into(),
        "-json".into(),
        format!("-o={o}"),
    ];
    cli::main(&args);
    assert!(PathBuf::from(&o).exists());
}
