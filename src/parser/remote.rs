//! `OpenAPIV3Parser.readContentFromLocation`: http(s) via `RemoteUrl`, `jar:` URLs, `file:` URIs and plain
//! paths, falling back to the class path.

use crate::core::AuthorizationValue;
use crate::java::path::JavaPath;
use crate::java::{JResult, JavaException};
use crate::parser::path_utils::{path_of_uri, uri_create};
use crate::parser::ref_utils::load_file_from_classpath;
use crate::parser::remote_url::url_to_string;

/// The class name of `ReadContentException`, the exception thrown when a location cannot be read.
pub const READ_CONTENT_EXCEPTION: &str = "io.swagger.v3.parser.exception.ReadContentException";

/// `OpenAPIV3Parser.readContentFromLocation(location, auth)` (`ReadContentException` on failure)
pub fn read_content_from_location(location: &str, auth: &[AuthorizationValue]) -> JResult<String> {
    let adjusted = location.replace('\\', "/");
    let lower = adjusted.to_lowercase();
    let attempt = || -> JResult<String> {
        if lower.starts_with("http") {
            url_to_string(&adjusted, auth, None)
        } else if lower.starts_with("jar:") {
            read_jar_url(&adjusted)
        } else {
            let path = if lower.starts_with("file:") { path_of_uri(&uri_create(&adjusted)?)? } else { JavaPath::new(&adjusted) };
            if path.exists() {
                read_file_to_string(&path)
            } else {
                load_file_from_classpath(&adjusted)
            }
        }
    };
    attempt().map_err(|e| {
        if e.class == "javax.net.ssl.SSLHandshakeException" {
            JavaException::new(
                READ_CONTENT_EXCEPTION,
                format!(
                    "Unable to read location `{adjusted}` due to a SSL configuration error. It is possible that the server SSL certificate is invalid, self-signed, or has an untrusted Certificate Authority."
                ),
            )
            .with_cause(e)
        } else {
            JavaException::new(READ_CONTENT_EXCEPTION, format!("Unable to read location `{adjusted}`")).with_cause(e)
        }
    })
}

static ENCODING: parking_lot::RwLock<Option<String>> = parking_lot::RwLock::new(None);

/// `OpenAPIV3Parser.getEncoding()` (process-wide, `UTF-8` by default)
pub fn get_encoding() -> String {
    ENCODING.read().clone().unwrap_or_else(|| "UTF-8".into())
}

/// `OpenAPIV3Parser.setEncoding(encoding)`: `EncodingNotSupportedException` for an unknown charset
pub fn set_encoding(encoding: &str) -> JResult<()> {
    if canonical_charset(encoding).is_none() {
        return Err(JavaException::new("io.swagger.v3.parser.exception.EncodingNotSupportedException", format!("Encoding `{encoding}` is not supported by JRE")));
    }
    *ENCODING.write() = Some(encoding.to_string());
    Ok(())
}

fn canonical_charset(name: &str) -> Option<&'static str> {
    match name.to_ascii_uppercase().replace('_', "-").as_str() {
        "UTF-8" | "UTF8" => Some("UTF-8"),
        "ISO-8859-1" | "ISO8859-1" | "LATIN1" | "ISO-LATIN-1" | "8859-1" | "CP819" | "L1" => Some("ISO-8859-1"),
        "US-ASCII" | "ASCII" => Some("US-ASCII"),
        _ => None,
    }
}

/// Decodes bytes with the configured encoding (malformed input replaced)
pub fn decode(bytes: &[u8]) -> String {
    match canonical_charset(&get_encoding()) {
        Some("ISO-8859-1") => bytes.iter().map(|&b| b as char).collect(),
        Some("US-ASCII") => bytes.iter().map(|&b| if b < 128 { b as char } else { '\u{fffd}' }).collect(),
        _ => String::from_utf8_lossy(bytes).into_owned(),
    }
}

/// `FileUtils.readFileToString(file, encoding)`
fn read_file_to_string(path: &JavaPath) -> JResult<String> {
    if path.is_directory() {
        return Err(JavaException::new("java.io.IOException", format!("File '{path}' exists but is a directory")));
    }
    std::fs::read(path.as_str()).map(|b| decode(&b)).map_err(|e| JavaException::new("java.io.IOException", e.to_string()))
}

/// `new URI(location).toURL().openStream()` for `jar:<file url>!/<entry>`
pub fn read_jar_url(location: &str) -> JResult<String> {
    let rest = &location[4..];
    let Some(sep) = rest.find("!/") else {
        return Err(JavaException::new("java.net.MalformedURLException", "no !/ in spec".to_string()));
    };
    let archive = &rest[..sep];
    let entry = &rest[sep + 2..];
    let archive_path = if archive.to_lowercase().starts_with("file:") { path_of_uri(&uri_create(archive)?)? } else { JavaPath::new(archive) };
    read_zip_entry(&archive_path, entry)
}

/// Reads one entry of a zip/jar archive as UTF-8.
pub fn read_zip_entry(archive: &JavaPath, entry: &str) -> JResult<String> {
    use std::io::Read;
    let f = std::fs::File::open(archive.as_str()).map_err(|e| JavaException::new("java.nio.file.NoSuchFileException", format!("{archive}: {e}")))?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| JavaException::new("java.util.zip.ZipException", e.to_string()))?;
    let mut file = z.by_name(entry).map_err(|_| JavaException::new("java.io.FileNotFoundException", format!("JAR entry {entry} not found in {archive}")))?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).map_err(|e| JavaException::new("java.io.IOException", e.to_string()))?;
    Ok(decode(&buf))
}
