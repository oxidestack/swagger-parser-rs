//! `io.swagger.v3.parser.util.PathUtils` and the `Paths.get(URI)` / `Path.toUri()` behaviour it relies on.

use crate::java::classpath::find_class_resource;
use crate::java::net::JavaUri;
use crate::java::path::JavaPath;
use crate::java::{JResult, JavaException};

fn iae(msg: impl Into<String>) -> JavaException {
    JavaException::new("java.lang.IllegalArgumentException", msg)
}

/// `URI.create(s)`: `IllegalArgumentException` wrapping the `URISyntaxException`
pub fn uri_create(s: &str) -> JResult<JavaUri> {
    JavaUri::parse(s).map_err(|e| iae(e.message.clone().unwrap_or_default()).with_cause(e))
}

/// `Paths.get(URI)` (the default file system provider)
pub fn path_of_uri(uri: &JavaUri) -> JResult<JavaPath> {
    let Some(scheme) = uri.get_scheme() else { return Err(iae("Missing scheme")) };
    if !scheme.eq_ignore_ascii_case("file") {
        return Err(JavaException::new("java.nio.file.FileSystemNotFoundException", format!("Provider \"{scheme}\" not installed")));
    }
    if uri.is_opaque() {
        return Err(iae("URI is not hierarchical"));
    }
    if uri.get_raw_authority().is_some() {
        return Err(iae("URI has an authority component"));
    }
    if uri.get_fragment().is_some() {
        return Err(iae("URI has a fragment component"));
    }
    if uri.get_query().is_some() {
        return Err(iae("URI has a query component"));
    }
    let raw = uri.get_raw_path().unwrap_or("");
    if raw.is_empty() {
        return Err(iae("URI path component is empty"));
    }
    let decoded = crate::java::path::percent_decode(raw);
    Ok(JavaPath::new(&decoded))
}

/// `Path.toUri()` as a `java.net.URI`
pub fn path_to_uri(p: &JavaPath) -> JavaUri {
    JavaUri::parse(&p.to_uri()).expect("Path.toUri yields a valid URI")
}

/// `getParentDirectoryOfFile(location)`
pub fn get_parent_directory_of_file(location: &str) -> JResult<Option<JavaPath>> {
    let location = location.replace('\\', "/");
    let file: JResult<JavaPath> = (|| {
        if location.to_lowercase().starts_with("file:") {
            Ok(path_of_uri(&uri_create(&location)?)?.to_absolute_path())
        } else {
            Ok(JavaPath::new(&location).to_absolute_path())
        }
    })();
    match file {
        Ok(f) => {
            if !f.exists() {
                return Ok(get_parent_directory_from_url(&location));
            }
            Ok(f.to_absolute_path().get_parent())
        }
        // `file` stays null and `file.toAbsolutePath()` throws
        Err(_) => Err(JavaException::npe("Cannot invoke \"java.nio.file.Path.toAbsolutePath()\" because \"file\" is null")),
    }
}

fn get_parent_directory_from_url(location: &str) -> Option<JavaPath> {
    get_path_from_classpath(location).and_then(|p| p.get_parent())
}

/// `getPathFromClasspath(location)` (`None` where Java throws)
fn get_path_from_classpath(location: &str) -> Option<JavaPath> {
    find_class_resource(location).map(|p| p.to_absolute_path())
}

/// `getAbsoluteUrl(location)`
pub fn get_absolute_url(location: &str) -> String {
    let location = location.replace('\\', "/");
    let lower = location.to_lowercase();
    if lower.starts_with("http:") || lower.starts_with("https:") {
        return location;
    }
    let file: JResult<JavaPath> = (|| {
        if lower.starts_with("file:") {
            Ok(path_of_uri(&uri_create(&location)?)?.to_absolute_path())
        } else {
            Ok(JavaPath::new(&location).to_absolute_path())
        }
    })();
    match file {
        Err(_) => location,
        Ok(f) => {
            if !f.exists() {
                return match get_path_from_classpath(&location) {
                    Some(p) => p.to_absolute_path().to_uri(),
                    None => location,
                };
            }
            f.to_absolute_path().to_uri()
        }
    }
}

/// `rootDocumentUri(rootPath)`
pub fn root_document_uri(root_path: Option<&str>) -> Option<JavaUri> {
    let root_path = root_path.filter(|r| !r.is_empty())?;
    let c: Vec<char> = root_path.chars().collect();
    if c.len() >= 3 && c[0].is_alphabetic() && c[1] == ':' && (c[2] == '/' || c[2] == '\\') {
        return existing_file_uri(root_path);
    }
    if let Ok(u) = JavaUri::parse(root_path) && u.get_scheme().is_some() {
        return normalize_root_document_uri(&u);
    }
    existing_file_uri(root_path)
}

fn normalize_root_document_uri(u: &JavaUri) -> Option<JavaUri> {
    let http = is_http_uri(Some(u));
    let file = u.get_scheme().is_some_and(|s| s.eq_ignore_ascii_case("file"));
    if (!http && !file) || u.is_opaque() || (http && u.get_raw_authority().is_none_or(|a| a.trim().is_empty())) {
        return None;
    }
    without_fragment(Some(&u.normalize()))
}

fn existing_file_uri(location: &str) -> Option<JavaUri> {
    if location.contains('\0') {
        return None;
    }
    let f = JavaPath::new(location);
    if f.is_regular_file() {
        return Some(path_to_uri(&f.to_absolute_path().normalize()));
    }
    None
}

/// `parentDirectoryOfUri(fileUri)`
pub fn parent_directory_of_uri(u: &JavaUri) -> Option<JavaPath> {
    // new URI(scheme, authority, path, null, null) then Paths.get
    let rebuilt = JavaUri::parse(&format!(
        "{}:{}{}",
        u.get_scheme().unwrap_or(""),
        u.get_raw_authority().map(|a| format!("//{a}")).unwrap_or_default(),
        u.get_raw_path().unwrap_or("")
    ))
    .ok()?;
    path_of_uri(&rebuilt).ok().map(|p| p.to_absolute_path()).and_then(|p| p.get_parent())
}

/// `isHttpUri(uri)`
pub fn is_http_uri(u: Option<&JavaUri>) -> bool {
    u.and_then(|u| u.get_scheme()).is_some_and(|s| s.eq_ignore_ascii_case("http") || s.eq_ignore_ascii_case("https"))
}

/// `withoutFragment(uri)`
pub fn without_fragment(u: Option<&JavaUri>) -> Option<JavaUri> {
    let u = u?;
    if u.is_opaque() {
        return None;
    }
    if u.get_fragment().is_none() {
        return Some(u.clone());
    }
    let value = u.to_java_string();
    let i = value.find('#')?;
    JavaUri::parse(&value[..i]).ok()
}
