//! Port of `io.swagger.v3.parser.util.RefUtilsZipFileSystemTest`: a relative ref read against a parent directory
//! that lives inside a zip file system.

mod common;

use std::io::Write;
use swagger_parser::java::path::JavaPath;
use swagger_parser::parser::ref_utils::{read_external_ref, RefFormat};
use swagger_parser_safe_url_resolver::PermittedUrlsChecker;

#[test]
fn test_read_external_ref_from_zip_file_system() {
    let _g = common::setup(common::V3_MODULE);
    // Files.createTempFile("ref-utils", ".zip"); Files.delete(archive)
    let archive = std::env::temp_dir().join(format!("ref-utils{}.zip", std::process::id()));
    let _ = std::fs::remove_file(&archive);
    let archive_str = archive.to_string_lossy().into_owned();
    let expected_content = "openapi: 3.0.0\n";

    // FileSystems.newFileSystem(jar:<archive>, {create=true}); createDirectories("/specs"); write the file
    {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&archive).unwrap());
        let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        zip.add_directory("specs/", options).unwrap();
        zip.start_file("specs/error.openapi.yaml", options).unwrap();
        zip.write_all(expected_content.as_bytes()).unwrap();
        zip.finish().unwrap();
    }
    let parent_directory = JavaPath::in_archive(&archive_str, "/specs");
    let referenced_file = parent_directory.resolve("error.openapi.yaml");
    assert!(referenced_file.exists());

    // assertNotSame(FileSystems.getDefault(), referencedFile.getFileSystem())
    assert!(referenced_file.archive().is_some());

    let actual = read_external_ref("./error.openapi.yaml", RefFormat::Relative, &[], Some(&parent_directory), Some(&PermittedUrlsChecker::default()));
    let _ = std::fs::remove_file(&archive);
    assert_eq!(Some(expected_content.to_string()), actual.unwrap());
}
