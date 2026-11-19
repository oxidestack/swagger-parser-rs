//! `io.swagger.v3.parser.models.RefFormat` / `RefType` and `io.swagger.v3.parser.util.RefUtils`.

use crate::core::AuthorizationValue;
use crate::java::net::JavaUrl;
use crate::java::path::JavaPath;
use crate::java::{JResult, JavaException};
use crate::parser::{hooks, remote_url};
use swagger_parser_safe_url_resolver::PermittedUrlsChecker;

/// `RefFormat`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RefFormat {
    /// A reference to a remote document (`http:`/`https:` and other URLs).
    Url,
    /// A reference to a file relative to the current document.
    Relative,
    /// A reference inside the current document (`#/...`).
    Internal,
}

impl RefFormat {
    /// `name()` / `toString()`
    pub fn name(&self) -> &'static str {
        match self {
            RefFormat::Url => "URL",
            RefFormat::Relative => "RELATIVE",
            RefFormat::Internal => "INTERNAL",
        }
    }
}

impl std::fmt::Display for RefFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// `RefType`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefType {
    /// `RefType.COMPONENTS`: `#/components/`.
    Components,
    /// `RefType.SCHEMAS`: `#/components/schemas/`.
    Schemas,
    /// `RefType.PATH`: `#/paths/`.
    Path,
}

impl RefType {
    /// `getInternalPrefix()`: the local reference prefix of this type.
    pub fn internal_prefix(&self) -> &'static str {
        match self {
            RefType::Components => "#/components/",
            RefType::Schemas => "#/components/schemas/",
            RefType::Path => "#/paths/",
        }
    }
}

const REFERENCE_SEPARATOR: &str = "#/";

fn runtime(msg: impl Into<String>) -> JavaException {
    JavaException::new("java.lang.RuntimeException", msg)
}

/// `String.split(regex)` for a literal separator: trailing empty strings removed.
pub fn java_split(s: &str, sep: &str) -> Vec<String> {
    if s.is_empty() {
        return vec![String::new()];
    }
    let mut v: Vec<String> = s.split(sep).map(str::to_string).collect();
    // a leading zero-width match never happens for a non-empty literal separator
    while v.last().is_some_and(|x| x.is_empty()) {
        v.pop();
    }
    v
}

/// `computeDefinitionName(ref)`
pub fn compute_definition_name(r: &str) -> JResult<String> {
    let parts = java_split(r, REFERENCE_SEPARATOR);
    if parts.len() > 2 {
        return Err(runtime(format!("Invalid ref format: {r}")));
    }
    let file = parts.first().cloned().unwrap_or_default();
    let definition_path = if parts.len() == 2 { Some(parts[1].clone()) } else { None };
    let array_last = |v: Vec<String>, what: &str| -> JResult<String> {
        v.last().cloned().ok_or_else(|| JavaException::new("java.lang.ArrayIndexOutOfBoundsException", format!("Index -1 out of bounds for length 0{what}")))
    };
    Ok(match definition_path {
        Some(dp) => array_last(java_split(&dp, "/"), "")?,
        None => {
            if parts.is_empty() {
                return Err(JavaException::new("java.lang.ArrayIndexOutOfBoundsException", "Index 0 out of bounds for length 0"));
            }
            let plausible = array_last(java_split(&file, "/"), "")?;
            let split = java_split(&plausible, ".");
            if split.len() > 2 {
                split[..split.len() - 1].join("")
            } else {
                match split.first() {
                    Some(s) => s.clone(),
                    None => return Err(JavaException::new("java.lang.ArrayIndexOutOfBoundsException", "Index 0 out of bounds for length 0")),
                }
            }
        }
    })
}

/// `getExternalPath(ref)`
pub fn get_external_path(r: Option<&str>) -> Option<String> {
    let r = r?;
    let parts = java_split(r, REFERENCE_SEPARATOR);
    if parts.len() == 2 && !parts[0].is_empty() { Some(parts[0].clone()) } else { None }
}

/// `isAnExternalRefFormat`
pub fn is_an_external_ref_format(f: RefFormat) -> bool {
    matches!(f, RefFormat::Url | RefFormat::Relative)
}

/// `computeRefFormat(ref)`
pub fn compute_ref_format(r: &str) -> RefFormat {
    let r = munged_ref(r);
    if r.starts_with("http") {
        RefFormat::Url
    } else if r.starts_with(REFERENCE_SEPARATOR) {
        RefFormat::Internal
    } else if r.starts_with('.') || r.starts_with('/') || r.find(REFERENCE_SEPARATOR).is_some_and(|i| i > 0) {
        RefFormat::Relative
    } else {
        RefFormat::Internal
    }
}

/// `mungedRef(refString)`
pub fn munged_ref(r: &str) -> String {
    if !r.contains(':') && !r.starts_with('#') && !r.starts_with('/') && !r.contains('$') && r.find('.').is_some_and(|i| i > 0) {
        return format!("./{r}");
    }
    r.to_string()
}

/// `RemoteUrl.urlToString(url, auths, checker)` through the hook
fn url_to_string(url: &str, auths: &[AuthorizationValue], checker: Option<&PermittedUrlsChecker>) -> JResult<String> {
    remote_url::url_to_string(url, auths, checker)
}

fn unable_to_load(format: RefFormat, file: &str, suffix: &str, cause: JavaException) -> JavaException {
    runtime(format!("Unable to load {format} ref: {file}{suffix}")).with_cause(cause)
}

/// `readExternalUrlRef(file, refFormat, auths, rootPath, checker)`
pub fn read_external_url_ref(file: &str, format: RefFormat, auths: &[AuthorizationValue], root_path: Option<&str>, checker: Option<&PermittedUrlsChecker>) -> JResult<Option<String>> {
    if let Some(h) = hooks::current().read_external_url_ref
        && let Some(r) = h(file, format, auths, root_path, checker)
    {
        return r;
    }
    if !is_an_external_ref_format(format) {
        return Err(runtime("Ref is not external"));
    }
    let attempt = || -> JResult<Option<String>> {
        if format == RefFormat::Url {
            url_to_string(file, auths, checker).map(Some)
        } else {
            let url = build_url(root_path, Some(file))?;
            match url {
                // readExternalRef(null, URL, ...) → RemoteUrl.urlToString(null) → NPE inside
                None => Err(JavaException::npe("Cannot invoke \"String.replace(java.lang.CharSequence, java.lang.CharSequence)\" because \"url\" is null")),
                Some(u) => read_external_ref(&u, RefFormat::Url, auths, None, checker),
            }
        }
    };
    attempt().map_err(|e| unable_to_load(format, file, "", e))
}

/// `readExternalClasspathRef(file, refFormat, auths, rootPath, checker)`
pub fn read_external_classpath_ref(file: &str, format: RefFormat, auths: &[AuthorizationValue], root_path: Option<&str>, checker: Option<&PermittedUrlsChecker>) -> JResult<Option<String>> {
    if let Some(h) = hooks::current().read_external_classpath_ref
        && let Some(r) = h(file, format, auths, root_path, checker)
    {
        return r;
    }
    if !is_an_external_ref_format(format) {
        return Err(runtime("Ref is not external"));
    }
    let attempt = || -> JResult<Option<String>> {
        if format == RefFormat::Url {
            url_to_string(file, auths, checker).map(Some)
        } else {
            let path_ref = crate::parser::processors::reference_path_utils::resolve(root_path, Some(file));
            match path_ref {
                Some(p) => load_file_from_classpath(&p).map(Some),
                None => Err(JavaException::npe("Cannot invoke \"String.length()\" because \"path\" is null")),
            }
        }
    };
    attempt().map_err(|e| unable_to_load(format, file, "", e))
}

/// `buildUrl(rootPath, relativePath)`
pub fn build_url(root_path: Option<&str>, relative_path: Option<&str>) -> JResult<Option<String>> {
    let (Some(root_path), Some(relative)) = (root_path, relative_path) else { return Ok(None) };
    let until = root_path.rfind('/').map(|i| i + 1).unwrap_or(0);
    let root = &root_path[..until];
    let attempt = || -> JResult<String> {
        let root_url = JavaUrl::parse(root)?;
        let final_url = JavaUrl::parse_with_context(Some(&root_url), relative)?;
        Ok(final_url.to_external_form())
    };
    attempt().map(Some).map_err(JavaException::runtime_wrapping)
}

/// `readExternalRef(file, refFormat, auths, parentDirectory, checker)`
pub fn read_external_ref(file: &str, format: RefFormat, auths: &[AuthorizationValue], parent_directory: Option<&JavaPath>, checker: Option<&PermittedUrlsChecker>) -> JResult<Option<String>> {
    if let Some(h) = hooks::current().read_external_ref {
        let parent = parent_directory.map(|p| p.to_string());
        if let Some(r) = h(file, format, auths, parent.as_deref(), checker) {
            return r;
        }
    }
    if !is_an_external_ref_format(format) {
        return Err(runtime("Ref is not external"));
    }
    let mut parent_dir: Option<JavaPath> = parent_directory.cloned();
    let attempt = |parent_dir: &mut Option<JavaPath>| -> JResult<Option<String>> {
        if format == RefFormat::Url {
            return url_to_string(file, auths, checker).map(Some);
        }
        let Some(pd) = parent_dir.clone() else {
            return Err(JavaException::npe("Cannot invoke \"java.nio.file.Path.resolve(String)\" because \"parentDirectory\" is null"));
        };
        let mut result: Option<String> = None;
        let path_to_use = pd.resolve(file).normalize();
        if path_to_use.exists() {
            result = Some(read_all(&path_to_use)?);
        } else {
            let mut url = file.to_string();
            if url.contains("..") {
                let mut parent_count = 0;
                while url.contains("..") {
                    let i = url.find('.').unwrap();
                    if i + 2 > url.len() {
                        return Err(JavaException::new(
                            "java.lang.StringIndexOutOfBoundsException",
                            format!("begin {}, end {}, length {}", i + 2, url.len(), url.len()),
                        ));
                    }
                    url = url[i + 2..].to_string();
                    parent_count += 1;
                }
                let mut p = pd.clone();
                for _ in 0..parent_count - 1 {
                    match p.get_parent() {
                        Some(x) => p = x,
                        None => {
                            *parent_dir = None;
                            return Err(JavaException::npe("Cannot invoke \"java.nio.file.Path.getParent()\" because \"parentDirectory\" is null"));
                        }
                    }
                }
                *parent_dir = Some(p.clone());
                url = format!("{p}{url}");
            } else {
                let i = url.find('.').map(|i| i as i64).unwrap_or(-1);
                let start = (i + 1) as usize;
                url = format!("{pd}{}", &url[start.min(url.len())..]);
            }
            let p = parent_dir.clone().unwrap();
            let path_to_use2 = p.resolve(&url).normalize();
            if path_to_use2.exists() {
                result = Some(read_all(&path_to_use2)?);
            }
        }
        if result.is_none() {
            result = Some(load_file_from_classpath(file)?);
        }
        Ok(result)
    };
    match attempt(&mut parent_dir) {
        Ok(r) => Ok(r),
        Err(e) => {
            let pd = parent_dir.map(|p| p.to_string()).unwrap_or_else(|| "null".into());
            Err(unable_to_load(format, file, &format!(" path: {pd}"), e))
        }
    }
}

/// `Files.newInputStream(path)` + `IOUtils.toString(in, UTF_8)` (a directory fails with an `IOException`)
fn read_all(path: &JavaPath) -> JResult<String> {
    if path.is_directory() {
        return Err(JavaException::new("java.io.IOException", "Is a directory"));
    }
    path.read_to_string().map_err(|e| JavaException::new("java.nio.file.NoSuchFileException", format!("{path}: {e}")))
}

/// `ClasspathHelper.loadFileFromClasspath(location)`
pub fn load_file_from_classpath(location: &str) -> JResult<String> {
    if let Some(h) = hooks::current().load_file_from_classpath && let Some(r) = h(location) {
        return r;
    }
    let file = location.replace('\\', "/");
    match crate::java::classpath::find_class_resource(&file) {
        Some(p) => p.read_to_string().map_err(|e| runtime(format!("Could not read {file} from the classpath")).with_cause(JavaException::new("java.io.IOException", e.to_string()))),
        None => Err(runtime(format!("Could not find {file} on the classpath"))),
    }
}
